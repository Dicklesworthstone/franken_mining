//! BD-062: independent exact known answers and source/interpretation controls for production GEO.
//! The declared worksheet is invented; these tests do not reproduce an issuer's production report.

use fmine_claim::{Claim, Confidentiality, Representation, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
use fmine_metrics::definitions::geo::{self, GeoInput, GeoReceipt, GeoValue, Metal, worksheet};
use fmine_units::{Currency, ExactQty};

const EXAMPLE: &str = include_str!("production_geo.toml");

fn evaluate(text: &str) -> Result<GeoReceipt, fmine_evidence::Refusal> {
    worksheet::evaluate(
        text,
        SourceAnchor::new(
            "invented_geo.toml",
            sha256(text.as_bytes()).0,
            "complete declared worksheet",
        ),
    )
}

fn qty(n: i128) -> ExactQty {
    ExactQty::from_integer(n)
}

fn rebind(input: &Claim<GeoInput>, value: GeoInput) -> Claim<GeoInput> {
    let tag = value.canonical_tag();
    let additivity = value.additivity();
    Claim::new(
        geo::INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        input.evidence,
        input.epistemic,
        input.state,
        input.confidentiality.clone(),
        input.uu_terms.clone(),
        input.parents.clone(),
        input.anchors.clone(),
    )
}

#[test]
fn gb27_preserves_physical_quantities_and_explains_only_the_price_ratio_difference() {
    let receipt = evaluate(EXAMPLE).unwrap();
    let s = receipt.summary();
    assert_eq!(s.metals, vec![Metal::Gold, Metal::Copper]);
    assert_eq!(s.excluded_metals, vec![Metal::Silver]);
    let budget = &s.decks[0];
    let comparison = &s.decks[1];
    // Independent calculation: Cu contribution = 440,000,000 * (4.5 / Au price).
    assert_eq!(budget.total_geo_oz, qty(3_860_000));
    assert_eq!(comparison.total_geo_oz, qty(3_640_000));
    assert_eq!(budget.difference_from_reference_oz, qty(0));
    assert_eq!(comparison.difference_from_reference_oz, qty(-220_000));
    assert_eq!(budget.contributions[0].geo_oz, qty(3_200_000));
    assert_eq!(budget.contributions[1].geo_oz, qty(660_000));
    assert_eq!(comparison.contributions[1].geo_oz, qty(440_000));
    for (a, b) in budget.contributions.iter().zip(&comparison.contributions) {
        assert_eq!(a.amount, b.amount);
        assert_eq!(a.quantity_claim_id, b.quantity_claim_id);
    }
    assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(receipt.claim().confidentiality, Confidentiality::internal());
    assert_eq!(receipt.claim().parents.len(), receipt.inputs().len());
    receipt.claim().check_id().unwrap();
    assert!(receipt.claim().verify_uu_slot_for_decision().is_err());
    for input in receipt.inputs() {
        input.check_id().unwrap();
        assert_eq!(input.evidence, EvidenceClass::Assumed);
        assert_eq!(
            input.epistemic,
            EpistemicStatus::KnownUnknown(KuKind::Parametric)
        );
        assert_eq!(input.state, KnowledgeState::Known);
        assert!(
            input
                .anchors
                .iter()
                .all(|a| a.artifact_sha256 == sha256(EXAMPLE.as_bytes()).0)
        );
    }
    assert!(
        receipt
            .to_json()
            .contains("\"calibration\":\"not_established\"")
    );
    assert!(
        receipt
            .to_json()
            .contains("\"metals_outside_declared_population\":[\"silver\"]")
    );
    assert!(receipt.to_markdown().contains("price-ratio effects only"));
    assert!(receipt.to_markdown().contains("-220000"));
}

#[test]
fn silver_requires_a_price_in_every_deck_and_contributes_exactly() {
    let with_silver = EXAMPLE.replace(
        "metals = \"gold,copper\"",
        "metals = \"gold,copper,silver\"",
    ) + "\n[[quantity]]\nmetal = \"silver\"\nunit = \"oz\"\namount = \"1000000\"\n";
    assert_eq!(evaluate(&with_silver).unwrap_err().code, "FMINE-REF-201");
    let with_prices = with_silver
        .replace(
            "gold_price_per_oz = \"3000\"",
            "gold_price_per_oz = \"3000\"\nsilver_price_per_oz = \"30\"",
        )
        .replace(
            "gold_price_per_oz = \"4500\"",
            "gold_price_per_oz = \"4500\"\nsilver_price_per_oz = \"45\"",
        );
    let receipt = evaluate(&with_prices).unwrap();
    assert!(receipt.summary().excluded_metals.is_empty());
    assert_eq!(receipt.summary().decks[0].total_geo_oz, qty(3_870_000));
    assert_eq!(receipt.summary().decks[1].total_geo_oz, qty(3_650_000));
    assert_eq!(
        receipt.summary().decks[1].difference_from_reference_oz,
        qty(-220_000)
    );
    for deck in &receipt.summary().decks {
        assert_eq!(
            deck.contributions[2].gold_oz_per_unit,
            ExactQty::new(1, 100).unwrap()
        );
        assert_eq!(deck.contributions[2].geo_oz, qty(10_000));
    }
    let missing_one = with_prices.replace("silver_price_per_oz = \"45\"\n", "");
    assert_eq!(evaluate(&missing_one).unwrap_err().code, "FMINE-REF-201");
}

#[test]
fn declaration_population_and_units_are_never_completed_or_converted_implicitly() {
    for (from, to, code) in [
        (
            "metals = \"gold,copper\"",
            "metals = \"gold\"",
            "FMINE-REF-201",
        ),
        (
            "metals = \"gold,copper\"",
            "metals = \"gold,copper,gold\"",
            "FMINE-REF-105",
        ),
        ("unit = \"lb\"", "unit = \"kt\"", "FMINE-REF-107"),
        ("unit = \"oz\"", "unit = \"koz\"", "FMINE-REF-107"),
        ("amount = \"440000000\"", "amount = \"-1\"", "FMINE-REF-109"),
        (
            "gold_price_per_oz = \"3000\"",
            "gold_price_per_oz = \"0\"",
            "FMINE-REF-109",
        ),
        (
            "reference_version = \"example-v1\"",
            "reference_version = \"missing\"",
            "FMINE-REF-201",
        ),
    ] {
        assert_eq!(
            evaluate(&EXAMPLE.replace(from, to)).unwrap_err().code,
            code,
            "{to}"
        );
    }
    let zero = evaluate(&EXAMPLE.replace("amount = \"440000000\"", "amount = \"0\"")).unwrap();
    assert!(
        zero.summary()
            .decks
            .iter()
            .all(|deck| deck.total_geo_oz == qty(3_200_000))
    );
    let attributable = evaluate(&EXAMPLE.replace(
        "ownership = \"full\"",
        "ownership = \"attributable\"\nownership_share = \"3/5\"",
    ))
    .unwrap();
    assert_eq!(attributable.summary().decks[0].total_geo_oz, qty(3_860_000));
    // Full and attributable declarations change identities, not the already-declared quantities.
    assert_ne!(attributable.digest(), evaluate(EXAMPLE).unwrap().digest());
}

#[test]
fn malformed_worksheets_refuse_unknown_duplicate_missing_and_unquoted_fields() {
    let examples = [
        EXAMPLE.replace("schema =", "evidence = \"Measured\"\nschema ="),
        EXAMPLE.replace("subject =", "subject = \"duplicate\"\nsubject ="),
        EXAMPLE.replace(
            "population = \"explicitly declared gold and copper production\"\n",
            "",
        ),
        EXAMPLE.replace("[[quantity]]", "[[unknown]]"),
        EXAMPLE.replace("amount = \"3200000\"", "amount = 3200000"),
        EXAMPLE.replace("amount = \"3200000\"", "amount = \"NaN\""),
        EXAMPLE.replace("amount = \"3200000\"", "amount = \"1/0\""),
        EXAMPLE.replace("period = \"2030FY\"", "period = \"2030Q5\""),
        EXAMPLE.replace("currency = \"USD\"", "currency = \"XYZ\""),
        EXAMPLE.replace(
            "ownership = \"full\"",
            "ownership = \"full\"\nownership_share = \"1/2\"",
        ),
    ];
    for text in examples {
        assert_eq!(evaluate(&text).unwrap_err().code, "FMINE-REF-506", "{text}");
    }
    let single = EXAMPLE
        .split("[[deck]]")
        .take(2)
        .collect::<Vec<_>>()
        .join("[[deck]]");
    assert_eq!(evaluate(&single).unwrap_err().code, "FMINE-REF-201");
    let duplicate = EXAMPLE.replace("id = \"comparison\"", "id = \"budget\"");
    assert_eq!(evaluate(&duplicate).unwrap_err().code, "FMINE-REF-105");
}

#[test]
fn core_admission_rechecks_scope_canonical_body_and_semantic_metadata() {
    let receipt = evaluate(EXAMPLE).unwrap();
    let index = receipt
        .inputs()
        .iter()
        .position(|c| matches!(c.value.value, GeoValue::Quantity { .. }))
        .unwrap();
    for field in 0..7 {
        let mut inputs = receipt.inputs().to_vec();
        let mut value = inputs[index].value.clone();
        match field {
            0 => value.scope.issuer.push_str("-other"),
            1 => value.scope.subject.push_str("-other"),
            2 => value.scope.period.year += 1,
            3 => value.scope.basis.push_str("-other"),
            4 => value.scope.population.push_str("-other"),
            5 => value.scope.currency = Currency::EUR,
            _ => {
                value.scope.ownership = fmine_metrics::guidance::Ownership::Attributable {
                    share: ExactQty::new(1, 2).unwrap(),
                }
            }
        }
        inputs[index] = rebind(&inputs[index], value);
        assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-105");
    }
    let mut inputs = receipt.inputs().to_vec();
    inputs[index].value.scope.subject.push_str("tampered");
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-507");
    let mut inputs = receipt.inputs().to_vec();
    inputs[index].stale = true;
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-502");
    let mut inputs = receipt.inputs().to_vec();
    inputs[index].support = SpatialSupport::PointCore;
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-102");
    let mut inputs = receipt.inputs().to_vec();
    inputs[index].calibration_failed = true;
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-202");
    let mut inputs = receipt.inputs().to_vec();
    inputs[index].anchors.clear();
    inputs[index] = rebind(&inputs[index], inputs[index].value.clone());
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-201");
}

#[test]
fn all_decks_must_be_usable_and_share_currency_without_silent_partial_output() {
    let original = evaluate(EXAMPLE).unwrap();
    let mut inputs = original.inputs().to_vec();
    let index = inputs
        .iter()
        .position(|c| matches!(&c.value.value, GeoValue::Deck(d) if d.key.id == "comparison"))
        .unwrap();
    let mut value = inputs[index].value.clone();
    if let GeoValue::Deck(deck) = &mut value.value {
        deck.currency = Currency::EUR;
    }
    inputs[index] = rebind(&inputs[index], value);
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-105");
    let overflow = EXAMPLE
        .replace(
            "amount = \"440000000\"",
            &format!("amount = \"{}\"", i128::MAX),
        )
        .replace("gold_price_per_oz = \"4500\"", "gold_price_per_oz = \"1\"");
    assert_eq!(evaluate(&overflow).unwrap_err().code, "FMINE-REF-207");
}

#[test]
fn arithmetic_inherits_the_weakest_evidence_state_and_strongest_confidentiality() {
    let original = evaluate(EXAMPLE).unwrap();
    let mut inputs = original.inputs().to_vec();
    for input in &mut inputs {
        input.confidentiality = Confidentiality::public();
        *input = rebind(input, input.value.clone());
    }
    inputs[0] = inputs[0].downgraded(EvidenceClass::ISSUER);
    inputs[0].state = KnowledgeState::Estimated;
    inputs[0].epistemic = EpistemicStatus::UnmodelledBudget;
    inputs[0].confidentiality = Confidentiality::internal();
    inputs[0] = rebind(&inputs[0], inputs[0].value.clone());
    let receipt = geo::evaluate(&inputs).unwrap();
    for (actual, prior) in receipt
        .summary()
        .decks
        .iter()
        .zip(&original.summary().decks)
    {
        assert_eq!(actual.total_geo_oz, prior.total_geo_oz);
        assert_eq!(
            actual.difference_from_reference_oz,
            prior.difference_from_reference_oz
        );
    }
    assert_eq!(receipt.claim().evidence, EvidenceClass::ISSUER);
    assert_eq!(receipt.claim().state, KnowledgeState::Estimated);
    assert_eq!(receipt.claim().epistemic, EpistemicStatus::UnmodelledBudget);
    assert_eq!(receipt.claim().confidentiality, Confidentiality::internal());
    assert_eq!(receipt.claim().color().unwrap_err().code, "FMINE-REF-205");
    assert!(
        receipt
            .claim()
            .verify_export_clearance(&Confidentiality::public())
            .is_err()
    );
    receipt.claim().check_id().unwrap();
    assert_ne!(receipt.digest(), original.digest());
}

#[test]
fn receipts_are_order_stable_but_bind_sources_deck_versions_and_unrendered_metadata() {
    let original = evaluate(EXAMPLE).unwrap();
    let mut inputs = original.inputs().to_vec();
    inputs.reverse();
    let reversed = geo::evaluate(&inputs).unwrap();
    assert_eq!(original, reversed);
    assert_eq!(original.to_json(), reversed.to_json());
    assert_eq!(original.to_markdown(), reversed.to_markdown());
    let different_bytes =
        evaluate(&(EXAMPLE.to_owned() + "\n# Different authenticated bytes.\n")).unwrap();
    assert_eq!(
        original.summary().decks[0].total_geo_oz,
        different_bytes.summary().decks[0].total_geo_oz
    );
    assert_ne!(original.digest(), different_bytes.digest());
    let bad_hash = worksheet::evaluate(
        EXAMPLE,
        SourceAnchor::new("invented_geo.toml", [1; 32], "whole file"),
    )
    .unwrap_err();
    assert_eq!(bad_hash.code, "FMINE-REF-505");
    let changed_version = evaluate(&EXAMPLE.replace("example-v1", "example-v2")).unwrap();
    assert_eq!(
        original.summary().decks[0].total_geo_oz,
        changed_version.summary().decks[0].total_geo_oz
    );
    assert_ne!(original.digest(), changed_version.digest());
    let mut inputs = original.inputs().to_vec();
    inputs[0].uu_terms.push(UuTerm {
        scenario_id: "omitted-price-regime".into(),
        description: "Invented unmodelled price-regime scenario".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Declared price relationship changes".into(),
    });
    let with_uu = geo::evaluate(&inputs).unwrap();
    assert_ne!(with_uu.digest(), original.digest());
    inputs[0].uu_terms[0].downside_exposure_usd_m = f64::from_bits(1.0_f64.to_bits() + 1);
    let changed_bits = geo::evaluate(&inputs).unwrap();
    assert_ne!(changed_bits.digest(), with_uu.digest());
    inputs[1].uu_terms.push(UuTerm {
        scenario_id: "omitted-price-regime".into(),
        description: "Conflicting definition".into(),
        downside_exposure_usd_m: 1.0,
        trigger_indicator: "Different trigger".into(),
    });
    assert_eq!(geo::evaluate(&inputs).unwrap_err().code, "FMINE-REF-204");
}
