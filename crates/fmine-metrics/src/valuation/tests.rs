//! Determinism: DeterministicPerIsa (FACT: synthetic known answers, no issuer prices or operating observations).

use super::*;
use fmine_plant::{cards, grinding, production};

const TERMS: &str = include_str!("../../../../tests/decisions/production_valuation.toml");

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
fn production() -> ProductionReceipt {
    let text = include_str!("../../../../tests/plant/grinding_joint.toml");
    let grinding = grinding::worksheet::evaluate(text, &source(text, "grinding.toml")).unwrap();
    let text = include_str!("../../../../tests/plant/production_recovery.toml");
    let recovery = cards::worksheet::evaluate(text, &source(text, "recovery.toml")).unwrap();
    let text = include_str!("../../../../tests/plant/production_joint.toml");
    production::worksheet::evaluate(
        text,
        &source(text, "production.toml"),
        grinding.receipt(),
        recovery.ensemble_receipt().unwrap(),
    )
    .unwrap()
    .receipt()
    .clone()
}
fn fixture(production: &ProductionReceipt) -> ValuationReceipt {
    worksheet::evaluate(TERMS, &source(TERMS, "terms.toml"), production).unwrap()
}
fn rebind(terms: &Claim<ValuationTerms>) -> Claim<ValuationTerms> {
    relabelled(terms, terms.confidentiality().clone())
}

/// `terms` rebuilt as a new claim under `label`. A label can rise on an existing claim, never fall, so a lower one
/// takes a new claim.
fn relabelled(terms: &Claim<ValuationTerms>, label: Confidentiality) -> Claim<ValuationTerms> {
    Claim::new(
        INPUT_METRIC,
        terms.value.clone(),
        &terms.value.canonical_tag(),
        terms.representation.clone(),
        terms.support,
        terms.additivity,
        terms.evidence,
        terms.epistemic,
        terms.state,
        label,
        terms.uu_terms.clone(),
        terms.parents.clone(),
        terms.anchors.clone(),
    )
}

#[test]
fn paired_cash_margins_retain_components_and_use_the_existing_decision_engine() {
    let production = production();
    let receipt = fixture(&production);
    assert_eq!(receipt.points.len(), 2);
    let hard = &receipt.points[0];
    let soft = &receipt.points[1];
    assert_eq!((&*hard.scenario_id, &*soft.scenario_id), ("hard", "soft"));
    assert_eq!(hard.represented_dry_tonnes, q(80, 1));
    assert_eq!(soft.represented_dry_tonnes, q(320, 1));
    assert_eq!(
        (hard.margin.minor, soft.margin.minor),
        (93_152_000, 8_080_000)
    );
    for p in receipt.points() {
        let operating = p
            .gross_revenue_usd
            .checked_sub(p.royalty_usd)
            .unwrap()
            .checked_sub(p.variable_cost_usd)
            .unwrap()
            .checked_sub(p.fixed_cost_usd)
            .unwrap();
        assert_eq!(operating, p.unrounded_margin_usd);
        assert_eq!(
            p.unrounded_margin_usd
                .checked_add(p.margin_rounding_delta_usd)
                .unwrap(),
            q(p.margin.minor, Money::SCALE)
        );
        assert!(
            p.margin_rounding_delta_usd
                .checked_cmp(q(-1, 20_000))
                .unwrap()
                .is_ge()
        );
        assert!(
            p.margin_rounding_delta_usd
                .checked_cmp(q(1, 20_000))
                .unwrap()
                .is_le()
        );
    }
    let s = receipt.decision.summary();
    let process = s
        .alternatives
        .iter()
        .find(|a| a.action_id == "process")
        .unwrap();
    assert_eq!(process.mean.major, q(25_308, 5));
    assert_eq!(process.cvar95_loss.major, q(-808, 1));
    assert_eq!(process.mean_regret.major, ZERO);
    assert_eq!(s.expected_value_winners, ["process"]);
    assert_eq!(
        receipt.decision,
        card::evaluate(receipt.decision.inputs()).unwrap()
    );
    assert!(
        receipt
            .to_json()
            .contains("\"submitted_worksheet\":\"# SYNTHETIC")
    );
    assert!(
        receipt
            .to_markdown()
            .contains("minus immediate cash margin")
    );
    assert!(!receipt.to_markdown().contains("−NPV"));
}

#[test]
fn price_and_production_are_paired_before_averaging() {
    let production = production();
    let receipt = fixture(&production);
    let half = q(1, 2);
    let revenue_mean = receipt.points.iter().fold(ZERO, |a, p| {
        a.checked_add(p.gross_revenue_usd.checked_mul(p.weight).unwrap())
            .unwrap()
    });
    let payable_mean = receipt.points.iter().fold(ZERO, |a, p| {
        a.checked_add(p.payable_kg.checked_mul(p.weight).unwrap())
            .unwrap()
    });
    let price_mean = receipt.terms.value.scenarios.iter().fold(ZERO, |a, s| {
        a.checked_add(s.usd_per_kg.checked_mul(s.weight).unwrap())
            .unwrap()
    });
    assert_ne!(revenue_mean, payable_mean.checked_mul(price_mean).unwrap());
    let mut swapped = receipt.terms.clone();
    let p0 = swapped.value.scenarios[0].usd_per_kg;
    swapped.value.scenarios[0].usd_per_kg = swapped.value.scenarios[1].usd_per_kg;
    swapped.value.scenarios[1].usd_per_kg = p0;
    let changed = evaluate(&production, &rebind(&swapped)).unwrap();
    assert_ne!(changed.decision.summary(), receipt.decision.summary());
    let independently_paired = receipt.points[0]
        .payable_kg
        .checked_mul(q(70_000, 1))
        .unwrap()
        .checked_add(
            receipt.points[1]
                .payable_kg
                .checked_mul(q(50_000, 1))
                .unwrap(),
        )
        .unwrap()
        .checked_mul(half)
        .unwrap();
    assert_eq!(independently_paired, revenue_mean);
}

#[test]
fn exact_binary64_conversion_and_signed_ties_even_are_explicit() {
    assert_eq!(
        represented(0.1).unwrap(),
        q(3_602_879_701_896_397, 36_028_797_018_963_968)
    );
    assert_eq!(represented(-0.0).unwrap(), ZERO);
    assert_eq!(represented(320.0).unwrap(), q(320, 1));
    for value in [f64::NAN, f64::INFINITY, -1.0, f64::MAX, f64::from_bits(1)] {
        assert_eq!(represented(value).unwrap_err().code, "FMINE-REF-207");
    }
    for (n, minor) in [(1, 0), (3, 2), (5, 2), (-1, 0), (-3, -2), (-5, -2)] {
        let input = q(n, 20_000);
        let (rounded, delta) = money(input, MonetaryRounding::NearestTiesEvenFourPlaces).unwrap();
        assert_eq!(rounded.minor, minor);
        assert_eq!(delta, q(minor, 10_000).checked_sub(input).unwrap());
    }
    assert!(money(q(i128::MAX, 1), MonetaryRounding::NearestTiesEvenFourPlaces).is_err());
}

#[test]
fn order_does_not_change_the_model_or_receipt_but_source_and_terms_do() {
    let production = production();
    let original = fixture(&production);
    let first = evaluate(&production, original.terms()).unwrap();
    let mut reordered = original.terms.clone();
    reordered.value.scenarios.reverse();
    let second = evaluate(&production, &rebind(&reordered)).unwrap();
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.to_json(), second.to_json());
    reordered.value.scenarios[0].fixed_usd = q(2_001, 1);
    assert_ne!(
        first.digest(),
        evaluate(&production, &rebind(&reordered)).unwrap().digest()
    );
    let mut changed_source = original.terms.clone();
    changed_source.anchors[0].locator.push_str(" revised");
    assert_ne!(
        first.digest(),
        evaluate(&production, &rebind(&changed_source))
            .unwrap()
            .digest()
    );
    let mut uu = original.terms.clone();
    uu.uu_terms[0].downside_exposure_usd_m *= 2.0;
    assert_ne!(
        first.digest(),
        evaluate(&production, &rebind(&uu)).unwrap().digest()
    );
    let mut superseding = original.terms.clone();
    superseding.supersedes = Some(production.claim().claim_id);
    assert_ne!(
        first.digest(),
        evaluate(&production, &superseding).unwrap().digest()
    );
    let mut physical_input = production.inputs().clone();
    physical_input.supersedes = Some(original.terms.claim_id);
    let changed_physical = production::evaluate(
        production.grinding(),
        production.recovery(),
        &physical_input,
    )
    .unwrap();
    assert_ne!(
        first.digest(),
        evaluate(&changed_physical, original.terms())
            .unwrap()
            .digest()
    );
}

#[test]
fn incomplete_unpaired_and_invalid_commercial_inputs_refuse_whole_population() {
    let production = production();
    let original = fixture(&production).terms;
    let checks: [fn(&mut ValuationTerms); 11] = [
        |t| {
            t.scenarios.pop();
        },
        |t| {
            t.scenarios[0].id = t.scenarios[1].id.clone();
        },
        |t| {
            t.scenarios[0].id = "foreign".into();
        },
        |t| {
            t.scenarios[0].weight = q(1, 3);
            t.scenarios[1].weight = q(2, 3);
        },
        |t| {
            t.scenarios[0].weight = q(0, 1);
        },
        |t| {
            t.scenarios[0].payable_fraction = q(11, 10);
        },
        |t| {
            t.scenarios[0].royalty_fraction = q(-1, 10);
        },
        |t| {
            t.scenarios[0].usd_per_kg = q(-1, 1);
        },
        |t| {
            t.scenarios[0].variable_usd_per_dry_t = q(-1, 1);
        },
        |t| {
            t.scenarios[0].fixed_usd = q(-1, 1);
        },
        |t| {
            t.action_id = DO_NOTHING.into();
        },
    ];
    for change in checks {
        let mut terms = original.clone();
        change(&mut terms.value);
        assert!(evaluate(&production, &rebind(&terms)).is_err());
    }
    for change in [
        |t: &mut ValuationTerms| {
            t.element = "Cu".into();
        },
        |t: &mut ValuationTerms| {
            t.boundary = "another mine".into();
        },
        |t: &mut ValuationTerms| {
            t.start_ns += 1;
        },
        |t: &mut ValuationTerms| {
            t.material_domain = "another domain".into();
        },
    ] {
        let mut terms = original.clone();
        change(&mut terms.value);
        assert_eq!(
            evaluate(&production, &rebind(&terms)).unwrap_err().code,
            "FMINE-REF-102"
        );
    }
    let mut mutation = original.clone();
    mutation.value.scenarios[0].usd_per_kg = ZERO;
    assert_eq!(
        evaluate(&production, &mutation).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut stale = original.clone();
    stale.stale = true;
    assert_eq!(
        evaluate(&production, &stale).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut no_uu = original.clone();
    no_uu.uu_terms.clear();
    assert_eq!(
        evaluate(&production, &rebind(&no_uu)).unwrap_err().code,
        "FMINE-REF-204"
    );
}

#[test]
fn evidence_sources_and_unknowns_survive_and_negative_margins_are_not_clamped() {
    let production = production();
    let original = fixture(&production);
    let mut terms = original.terms.clone();
    terms.evidence = EvidenceClass::Assumed;
    for scenario in &mut terms.value.scenarios {
        scenario.fixed_usd = q(100_000, 1);
        scenario.do_nothing_usd = q(-100, 1);
    }
    let terms = relabelled(&terms, Confidentiality::public());
    let result = evaluate(&production, &terms).unwrap();
    assert!(result.points.iter().all(|p| p.margin.minor < 0));
    assert_eq!(
        result.decision.summary().expected_value_winners,
        [DO_NOTHING]
    );
    let output = result.decision.claim();
    assert_eq!(output.evidence, EvidenceClass::Assumed);
    assert!(
        output
            .evidence
            .no_stronger_than(EvidenceClass::ModelledUncalibrated)
    );
    assert_eq!(*output.confidentiality(), Confidentiality::internal());
    assert_eq!(output.uu_terms, terms.uu_terms);
    for c in &result.decision.inputs().actions {
        assert!(c.parents.contains(&production.claim().claim_id));
        assert!(c.parents.contains(&terms.claim_id));
        for source in production.claim().anchors.iter().chain(&terms.anchors) {
            assert!(c.anchors.contains(source));
        }
    }
}

#[test]
fn physical_unknowns_cannot_replace_missing_commercial_unknowns() {
    let original = production();
    let mut input = original.inputs().clone();
    input.uu_terms.push(UuTerm {
        scenario_id: "physical-only".into(),
        description: "Synthetic unmodelled plant response".into(),
        trigger_indicator: "Recovery differs from the physical formula".into(),
        downside_exposure_usd_m: 1.0,
    });
    let production =
        production::evaluate(original.grinding(), original.recovery(), &input).unwrap();
    assert_eq!(production.claim().uu_terms.len(), 1);
    let mut terms = fixture(&original).terms;
    assert!(evaluate(&production, &terms).is_ok());
    terms.uu_terms.clear();
    assert_eq!(
        evaluate(&production, &rebind(&terms)).unwrap_err().code,
        "FMINE-REF-204"
    );
}

#[test]
fn worksheet_authenticates_sources_and_refuses_implicit_or_unknown_fields() {
    let production = production();
    let mut source = source(TERMS, "terms.toml");
    source.artifact_sha256[0] ^= 1;
    assert_eq!(
        worksheet::evaluate(TERMS, &source, &production)
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
    for changed in [
        TERMS.replace("nearest_ties_even_0_0001_usd", "silent_default"),
        TERMS.replace("usd_per_kg = \"50000\"", "usd_per_oz = \"50000\""),
        TERMS.replace("payable_fraction = \"0.9\"", "payable_fraction = 1"),
        TERMS.replace("immediate_one_period_cash_margin", "project_npv"),
    ] {
        assert!(
            worksheet::evaluate(
                &changed,
                &super::tests::source(&changed, "terms.toml"),
                &production
            )
            .is_err()
        );
    }
}
