//! FACT: synthetic independently calculated investment and rounding controls.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_plant::{cards, grinding, production};

const MANIFEST: &str = include_str!("../../../../../tests/decisions/production_investment.toml");
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
fn manifest(text: &str) -> worksheet::Manifest {
    worksheet::parse(text, &source(text, "investment.toml")).unwrap()
}
fn bundle(high: bool, second: bool, change: impl Fn(&str) -> String) -> PeriodValuation {
    let rewrite = |s: &str| {
        let s = change(s);
        if second {
            s.replace("end_ns = \"3600000000000\"", "end_ns = \"7200000000000\"")
                .replace("start_ns = \"0\"", "start_ns = \"3600000000000\"")
        } else {
            s
        }
    };
    let text = rewrite(if high { HIGH } else { LOW });
    let grind = grinding::worksheet::evaluate(&text, &source(&text, "grinding.toml")).unwrap();
    let text = rewrite(RECOVERY);
    let recovery = cards::worksheet::evaluate(&text, &source(&text, "recovery.toml")).unwrap();
    let text = rewrite(PRODUCTION);
    let physical = production::worksheet::evaluate(
        &text,
        &source(&text, "production.toml"),
        grind.receipt(),
        recovery.ensemble_receipt().unwrap(),
    )
    .unwrap();
    let text = rewrite(if high { HIGH_TERMS } else { LOW_TERMS });
    let valuation = crate::valuation::worksheet::evaluate(
        &text,
        &source(&text, "terms.toml"),
        physical.receipt(),
    )
    .unwrap();
    PeriodValuation {
        alternative_id: if high { "higher-power" } else { "process" }.into(),
        period_id: if second { "second" } else { "first" }.into(),
        valuation,
    }
}
fn bundles() -> Vec<PeriodValuation> {
    [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .map(|(h, s)| bundle(h, s, str::to_owned))
        .collect()
}
fn rebound(spec: InvestmentSpec, original: &Claim<InvestmentSpec>) -> Claim<InvestmentSpec> {
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
fn paired_period_cash_and_capex_reverse_the_operating_choice_without_transcribing_payoffs() {
    let inputs = bundles();
    let receipt = manifest(MANIFEST).evaluate(&inputs).unwrap();
    let summary = receipt.decision().summary();
    assert_eq!(summary.expected_value_winners, ["process"]);
    for (id, state, value) in [
        ("process", "hard", 129_728_000),
        ("process", "soft", 2_120_000),
        ("higher-power", "hard", 199_456_000),
        ("higher-power", "soft", -85_760_000),
    ] {
        let row = receipt
            .outcomes()
            .iter()
            .find(|p| p.alternative_id == id && p.scenario_id == state)
            .unwrap();
        assert_eq!(row.value.minor, value);
    }
    for (id, mean, tail) in [
        ("process", q(32962, 5), q(-212, 1)),
        ("higher-power", q(28424, 5), q(8576, 1)),
    ] {
        let a = summary
            .alternatives
            .iter()
            .find(|a| a.action_id == id)
            .unwrap();
        assert_eq!(a.mean.major, mean);
        assert_eq!(a.cvar95_loss.major, tail);
    }
    assert_eq!(receipt.cashflows().len(), 8);
    assert_eq!(
        receipt.bundles(),
        [
            inputs[2].clone(),
            inputs[3].clone(),
            inputs[0].clone(),
            inputs[1].clone()
        ]
    );
    assert_eq!(receipt.decision().claim().evidence, EvidenceClass::Assumed);
    assert_eq!(
        receipt.decision().claim().confidentiality,
        Confidentiality::internal()
    );
    assert!(
        receipt
            .decision()
            .claim()
            .uu_terms
            .iter()
            .any(|u| u.scenario_id == "UU-synthetic-investment-horizon")
    );
    assert_eq!(
        *receipt.decision(),
        card::evaluate(receipt.decision().inputs()).unwrap()
    );
    assert!(
        receipt
            .to_json()
            .contains("\"submitted_manifest\":\"# SYNTHETIC")
    );
    assert!(
        receipt
            .to_markdown()
            .contains("minus discounted incremental cash value")
    );
}

#[test]
fn explicit_discounting_changes_the_choice_and_negative_investments_retain_do_nothing() {
    let b = bundles();
    let undiscounted = MANIFEST.replace("discount_factor = \"1/2\"", "discount_factor = \"1\"");
    let receipt = manifest(&undiscounted).evaluate(&b).unwrap();
    assert_eq!(
        receipt.decision().summary().expected_value_winners,
        ["higher-power"]
    );
    let costly = MANIFEST
        .replace(
            "initial_capex_usd = \"1000\"",
            "initial_capex_usd = \"100000\"",
        )
        .replace(
            "initial_capex_usd = \"8000\"",
            "initial_capex_usd = \"100000\"",
        );
    let receipt = manifest(&costly).evaluate(&b).unwrap();
    assert_eq!(
        receipt.decision().summary().expected_value_winners,
        [DO_NOTHING]
    );
    assert_eq!(receipt.decision().summary().alternatives.len(), 3);
}

#[test]
fn unrounded_baselines_and_periods_reach_one_final_rounding_boundary() {
    let m = manifest(MANIFEST);
    let mut spec = m.declaration().value.clone();
    spec.alternatives.retain(|a| a.id == "process");
    spec.alternatives[0].initial_capex_usd = ZERO;
    for p in &mut spec.periods {
        p.discount_factor = ONE;
    }
    let terms = |s: &str| {
        s.replace("payable_fraction = \"0.9\"", "payable_fraction = \"0\"")
            .replace("payable_fraction = \"0.8\"", "payable_fraction = \"0\"")
            .replace(
                "variable_usd_per_dry_t = \"20\"",
                "variable_usd_per_dry_t = \"0\"",
            )
            .replace(
                "variable_usd_per_dry_t = \"40\"",
                "variable_usd_per_dry_t = \"0\"",
            )
            .replace("fixed_usd = \"1000\"", "fixed_usd = \"0\"")
            .replace("fixed_usd = \"2000\"", "fixed_usd = \"0\"")
            .replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.00003\"")
    };
    let b = [bundle(false, false, terms), bundle(false, true, terms)];
    assert!(
        b.iter()
            .flat_map(|b| b.valuation.points())
            .all(|p| p.margin.minor == 0 && p.do_nothing.minor == 0)
    );
    let receipt = evaluate(&rebound(spec, m.declaration()), &b).unwrap();
    for p in receipt.outcomes() {
        assert_eq!(p.unrounded_value_usd, q(-3, 50_000));
        assert_eq!(p.value.minor, -1);
        assert_eq!(p.rounding_delta_usd, q(-1, 25_000));
    }
}

#[test]
fn complete_state_paths_preserve_cross_period_and_price_volume_coupling() {
    let original = bundles();
    let m = manifest(MANIFEST);
    let before = m.evaluate(&original).unwrap();
    let mut paired = original.clone();
    paired[1] = bundle(false, true, |s| {
        s.replace("usd_per_kg = \"50000\"", "usd_per_kg = \"SWAP\"")
            .replace("usd_per_kg = \"70000\"", "usd_per_kg = \"50000\"")
            .replace("usd_per_kg = \"SWAP\"", "usd_per_kg = \"70000\"")
    });
    let after = m.evaluate(&paired).unwrap();
    assert_ne!(before.outcomes(), after.outcomes());
    assert_eq!(
        before.decision().inputs().weights.value.states,
        after.decision().inputs().weights.value.states
    );
    let mut crossed = original.clone();
    crossed[1] = bundle(false, true, |s| {
        s.replace("\"soft\"", "\"SWAP\"")
            .replace("\"hard\"", "\"soft\"")
            .replace("\"SWAP\"", "\"hard\"")
    });
    let crossed = m.evaluate(&crossed).unwrap();
    let original_stats = before
        .decision()
        .summary()
        .alternatives
        .iter()
        .find(|a| a.action_id == "process")
        .unwrap();
    let crossed_stats = crossed
        .decision()
        .summary()
        .alternatives
        .iter()
        .find(|a| a.action_id == "process")
        .unwrap();
    // Equal period marginals keep the mean but change complete-path downside.
    assert_eq!(original_stats.mean, crossed_stats.mean);
    assert_eq!(crossed_stats.cvar95_loss.major, q(-22328, 5));
    assert_ne!(original_stats.cvar95_loss, crossed_stats.cvar95_loss);
}

#[test]
fn malformed_support_states_baselines_and_incomplete_bundles_refuse_atomically() {
    let m = manifest(MANIFEST);
    let b = bundles();
    for change in [
        |s: &str| s.replace("\"soft\"", "\"foreign\""),
        |s: &str| {
            s.replacen("weight = \"1/2\"", "weight = \"1/4\"", 1)
                .replacen("weight = \"1/2\"", "weight = \"3/4\"", 1)
        },
        |s: &str| s.replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.000001\""),
        |s: &str| {
            s.replace(
                "with no opening stock",
                "with a changed basis and no opening stock",
            )
        },
    ] {
        let mut changed = b.clone();
        changed[3] = bundle(true, true, change);
        assert!(m.evaluate(&changed).is_err());
    }
    assert!(m.evaluate(&b[..3]).is_err());
    let mut repeated = b.clone();
    repeated[3] = b[2].clone();
    assert!(m.evaluate(&repeated).is_err());
    let mut foreign = b.clone();
    foreign[3].period_id = "foreign".into();
    assert!(m.evaluate(&foreign).is_err());
    let mut spec = m.declaration().value.clone();
    spec.periods[1].start_ns += 1;
    assert_eq!(
        evaluate(&rebound(spec, m.declaration()), &b)
            .unwrap_err()
            .code,
        "FMINE-REF-102"
    );
}

#[test]
fn declaration_validation_source_binding_and_canonical_order_are_preserved() {
    let b = bundles();
    let m = manifest(MANIFEST);
    let baseline = m.evaluate(&b).unwrap();
    let mut reordered = b.clone();
    reordered.reverse();
    assert_eq!(
        baseline.to_json(),
        m.evaluate(&reordered).unwrap().to_json()
    );
    let mut stale = m.declaration().clone();
    stale.stale = true;
    assert_eq!(evaluate(&stale, &b).unwrap_err().code, "FMINE-REF-502");
    let mut edited = m.declaration().clone();
    edited.value.alternatives[0].initial_capex_usd = ZERO;
    assert!(evaluate(&edited, &b).is_err());
    let mut overflow = m.declaration().value.clone();
    overflow.alternatives[0].initial_capex_usd = q(i128::MAX, 1);
    assert_eq!(
        evaluate(&rebound(overflow, m.declaration()), &b)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
    let mut uu = m.declaration().clone();
    uu.uu_terms[0].downside_exposure_usd_m *= 2.0;
    assert_ne!(baseline.digest(), evaluate(&uu, &b).unwrap().digest());
    assert!(worksheet::parse(MANIFEST, &source("altered", "manifest.toml")).is_err());
    for (from, to) in [
        ("currency = \"USD\"", "currency = \"CAD\""),
        ("valuation_time_ns = \"0\"", "valuation_time_ns = \"1\""),
        (
            "start_ns = \"3600000000000\"",
            "start_ns = \"3599999999999\"",
        ),
        ("discount_factor = \"1/2\"", "discount_factor = \"0\""),
        ("initial_capex_usd = \"1000\"", "initial_capex_usd = \"-1\""),
        ("period = \"second\"", "period = \"first\""),
        (
            "cashflow_basis = \"incremental_operating_cash_excludes_initial_capex\"",
            "cashflow_basis = \"unknown\"",
        ),
        ("downside_usd_m = \"0.03\"", "downside_usd_m = \"0\""),
    ] {
        let text = MANIFEST.replace(from, to);
        assert!(
            worksheet::parse(&text, &source(&text, "manifest.toml")).is_err(),
            "{from}"
        );
    }
}

fn limited(text: &str, floor: &str, maximum: &str) -> String {
    format!(
        "{text}\n[[downside_limit]]\nfloor_usd = \"{floor}\"\nmax_breach_probability = \"{maximum}\"\n"
    )
}

#[test]
fn loss_probability_selects_a_lower_return_investment_and_preserves_nominal_winners() {
    let text = MANIFEST.replace("discount_factor = \"1/2\"", "discount_factor = \"1\"");
    let receipt = manifest(&limited(&text, "0", "0"))
        .evaluate(&bundles())
        .unwrap();
    let summary = receipt.decision().summary();
    // Independently: 2*9123.2-8000 = 10246.4 versus 2*5061.6-1000 = 9123.2.
    assert_eq!(summary.expected_value_winners, ["higher-power"]);
    assert_eq!(summary.alternatives.len(), 3);
    let screen = summary.downside.as_ref().unwrap();
    assert_eq!(screen.nominal.winners.len(), 1);
    assert_eq!(screen.nominal.winners[0].id, "process");
    assert_eq!(
        screen.nominal.expected_value_cost_of_limit,
        Some(q(5616, 5))
    );
    for (id, probability, eligible) in [
        ("higher-power", q(1, 2), false),
        ("process", ZERO, true),
        (DO_NOTHING, ZERO, true),
    ] {
        let choice = screen.choices.iter().find(|c| c.choice.id == id).unwrap();
        assert_eq!(choice.nominal_breach_probability, probability);
        assert_eq!(choice.nominal_eligible, eligible);
    }
    let claim = receipt.decision().inputs().downside_limit.as_ref().unwrap();
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert!(claim.parents.contains(&receipt.declaration().claim_id));
    assert!(
        claim
            .anchors
            .iter()
            .any(|a| a.locator.contains("downside_limit.floor_usd"))
    );
    assert!(
        receipt
            .to_markdown()
            .contains("Downside-eligible expected-value co-winners:** process")
    );
    assert_eq!(
        *receipt.decision(),
        card::evaluate(receipt.decision().inputs()).unwrap()
    );
}

#[test]
fn downside_boundaries_are_inclusive_and_an_empty_selection_has_no_fallback() {
    let b = bundles();
    let equal_floor = manifest(&limited(MANIFEST, "212", "0"))
        .evaluate(&b)
        .unwrap();
    let screen = equal_floor.decision().summary().downside.as_ref().unwrap();
    assert_eq!(screen.nominal.winners[0].id, "process");
    assert_eq!(
        screen
            .choices
            .iter()
            .find(|c| c.choice.id == "process")
            .unwrap()
            .nominal_breach_probability,
        ZERO
    );
    let above = manifest(&limited(MANIFEST, "212.0001", "0"))
        .evaluate(&b)
        .unwrap();
    let screen = above.decision().summary().downside.as_ref().unwrap();
    assert_eq!(
        screen.nominal.status,
        card::downside::DownsideStatus::NoEligibleChoice
    );
    assert!(screen.nominal.winners.is_empty());
    assert_eq!(screen.nominal.expected_value_cost_of_limit, None);
    assert_eq!(screen.choices.len(), 3);
    assert!(above.to_markdown().contains("No eligible choice."));
    let text = MANIFEST.replace("discount_factor = \"1/2\"", "discount_factor = \"1\"");
    let equal_probability = manifest(&limited(&text, "0", "1/2")).evaluate(&b).unwrap();
    assert_eq!(
        equal_probability
            .decision()
            .summary()
            .downside
            .as_ref()
            .unwrap()
            .nominal
            .winners[0]
            .id,
        "higher-power"
    );
}

#[test]
fn downside_uses_final_discounted_value_after_capex_and_declared_rounding() {
    let m = manifest(&limited(MANIFEST, "0", "0"));
    let mut spec = m.declaration().value.clone();
    spec.alternatives.retain(|a| a.id == "process");
    spec.alternatives[0].initial_capex_usd = ZERO;
    for p in &mut spec.periods {
        p.discount_factor = ONE;
    }
    let zero_cash = |s: &str| {
        s.replace("payable_fraction = \"0.9\"", "payable_fraction = \"0\"")
            .replace("payable_fraction = \"0.8\"", "payable_fraction = \"0\"")
            .replace(
                "variable_usd_per_dry_t = \"20\"",
                "variable_usd_per_dry_t = \"0\"",
            )
            .replace(
                "variable_usd_per_dry_t = \"40\"",
                "variable_usd_per_dry_t = \"0\"",
            )
            .replace("fixed_usd = \"1000\"", "fixed_usd = \"0\"")
            .replace("fixed_usd = \"2000\"", "fixed_usd = \"0\"")
            .replace("do_nothing_usd = \"0\"", "do_nothing_usd = \"0.00002\"")
    };
    let b = [
        bundle(false, false, zero_cash),
        bundle(false, true, zero_cash),
    ];
    let receipt = evaluate(&rebound(spec, m.declaration()), &b).unwrap();
    assert!(
        receipt
            .outcomes()
            .iter()
            .all(|p| p.unrounded_value_usd == q(-1, 25_000) && p.value.minor == 0)
    );
    assert!(
        receipt
            .decision()
            .summary()
            .downside
            .as_ref()
            .unwrap()
            .choices
            .iter()
            .all(|c| c.nominal_breach_probability == ZERO)
    );
    let capex = manifest(&limited(MANIFEST, "500", "0"))
        .evaluate(&bundles())
        .unwrap();
    let process = capex
        .decision()
        .summary()
        .downside
        .as_ref()
        .unwrap()
        .choices
        .iter()
        .find(|c| c.choice.id == "process")
        .unwrap();
    // Each period's 808 USD soft-state margin exceeds 500; 808 + 808/2 - 1000 = 212 does not.
    assert_eq!(process.nominal_breach_probability, q(1, 2));
}

#[test]
fn downside_source_identity_and_legacy_declaration_are_stable() {
    let m = manifest(MANIFEST);
    assert_eq!(
        m.declaration().value.canonical_tag(),
        "production-investment/v1:24da1470520d06afa5c4493a2467ec3985a44d79312403e47aa772acb53d2d04"
    );
    let b = bundles();
    let original = m.evaluate(&b).unwrap();
    let constrained = manifest(&limited(MANIFEST, "0", "0")).evaluate(&b).unwrap();
    let changed = manifest(&limited(MANIFEST, "0", "1/2"))
        .evaluate(&b)
        .unwrap();
    assert_ne!(original.digest(), constrained.digest());
    assert_ne!(constrained.digest(), changed.digest());
    assert_eq!(constrained.outcomes(), changed.outcomes());
    assert!(!original.to_json().contains("\"downside_limit\""));
    let mut tampered = constrained.declaration().clone();
    tampered.value.downside_limit.as_mut().unwrap().floor.minor += 1;
    assert!(evaluate(&tampered, &b).is_err());
    for change in [
        |l: &mut DownsideLimit| l.model_key.push_str("foreign"),
        |l: &mut DownsideLimit| l.valuation_basis.push_str("foreign"),
        |l: &mut DownsideLimit| l.floor.ccy = Currency::CAD,
        |l: &mut DownsideLimit| l.max_breach_probability = q(3, 2),
        |l: &mut DownsideLimit| {
            l.policy_optimization = Some(card::downside::PolicyOptimization {
                screen: card::downside::PolicyScreen::Nominal,
                max_policies: 1,
                max_work: 1,
            });
        },
    ] {
        let mut spec = constrained.declaration().value.clone();
        change(spec.downside_limit.as_mut().unwrap());
        assert!(evaluate(&rebound(spec, constrained.declaration()), &b).is_err());
    }
}

#[test]
fn malformed_or_lossy_investment_limits_refuse_without_defaulting() {
    for (floor, probability) in [
        ("0.00001", "0"),
        ("1/3", "0"),
        ("0", "-1/2"),
        ("0", "3/2"),
        ("0", "1/0"),
        ("170141183460469231731687303715884105727", "0"),
    ] {
        let text = limited(MANIFEST, floor, probability);
        assert!(worksheet::parse(&text, &source(&text, "manifest.toml")).is_err());
    }
    let valid = limited(MANIFEST, "-1/8", "1/3");
    assert_eq!(
        manifest(&valid)
            .declaration()
            .value
            .downside_limit
            .as_ref()
            .unwrap()
            .floor
            .minor,
        -1250
    );
    for text in [
        limited(&valid, "0", "0"),
        valid.replace("floor_usd = \"-1/8\"", "floor_eur = \"-1/8\""),
        valid.replace("max_breach_probability = \"1/3\"", ""),
        format!("{valid}\n[[policy_optimization]]\nmethod = \"exact_deterministic\"\n"),
    ] {
        assert!(worksheet::parse(&text, &source(&text, "manifest.toml")).is_err());
    }
}
