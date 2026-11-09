//! Synthetic analytic Bond answers and adversarial source/domain controls.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, LabelRegistry, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::EpistemicStatus;

const NOMINAL: &str = include_str!("../../../../tests/plant/grinding_bond.toml");
const JOINT: &str = include_str!("../../../../tests/plant/grinding_joint.toml");

fn scope() -> GrindingScope {
    GrindingScope {
        boundary: "synthetic-boundary/v1".into(),
        circuit: "synthetic-ball-mill".into(),
        blend: "synthetic-hour-blend".into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: 3_600_000_000_000,
        },
    }
}
fn source<T>(metric: &str, value: T, tag: String) -> Claim<T> {
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Measured,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "synthetic/grinding-tests",
            sha256(include_bytes!("tests.rs")).0,
            "authored synthetic inputs",
        )],
    )
}
fn model_claim(m: BondModel) -> Claim<BondModel> {
    let tag = m.canonical_tag();
    source(MODEL_METRIC, m, tag)
}
fn scenario_claim(s: GrindingScenario) -> Claim<GrindingScenario> {
    let tag = s.canonical_tag();
    source(INPUT_METRIC, s, tag)
}
fn model() -> Claim<BondModel> {
    model_claim(BondModel {
        scope: scope(),
        id: "synthetic-bond".into(),
        version: "1".into(),
        work_index_test_basis:
            "synthetic metric work index; 150 micron closing screen and declared prepared feed"
                .into(),
        validity_basis: "synthetic assumed validity box".into(),
        complete_joint_population: true,
        work_index_domain: [KwhPerDryTonne::new(1.0), KwhPerDryTonne::new(100.0)],
        feed_size_domain: [Micrometres::new(100.0), Micrometres::new(2000.0)],
        product_size_domain: [Micrometres::new(1.0), Micrometres::new(500.0)],
        power_domain: [Kilowatts::new(0.0), Kilowatts::new(10_000.0)],
        utilization_domain: [Fraction::new(0.0), Fraction::new(1.0)],
    })
}
fn scenario(
    id: &str,
    wi: f64,
    power: f64,
    utilization: f64,
    weight: ExactQty,
) -> Claim<GrindingScenario> {
    scenario_claim(GrindingScenario {
        id: id.into(),
        scope: scope(),
        weight,
        work_index: KwhPerDryTonne::new(wi),
        feed_size: Micrometres::new(400.0),
        product_size: Micrometres::new(100.0),
        installed_power: Kilowatts::new(power),
        utilization: Fraction::new(utilization),
    })
}
fn one() -> ExactQty {
    ExactQty::from_integer(1)
}
fn half() -> ExactQty {
    ExactQty::new(1, 2).unwrap()
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(f64::MIN_POSITIVE) * 2e-14,
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}
fn anchor(text: &str) -> SourceAnchor {
    SourceAnchor::new(
        "synthetic/grinding.toml",
        sha256(text.as_bytes()).0,
        "worksheet",
    )
}
fn worksheet(text: &str) -> Result<worksheet::WorksheetResult> {
    worksheet::evaluate(text, &anchor(text))
}

#[test]
fn square_root_known_answers_produce_energy_and_power_limited_throughput() {
    // Independent hand arithmetic: 100*(1/10-1/20)=5 and 800/5=160.
    let result = evaluate(&model(), &[scenario("one", 10.0, 1000.0, 0.8, one())]).unwrap();
    close(result.points()[0].specific_energy.value(), 5.0);
    close(result.points()[0].effective_power.value(), 800.0);
    close(result.points()[0].throughput.value(), 160.0);
    assert_eq!(result.points()[0].state, OperatingState::Operating);
    assert_eq!(result.summary().scenario_count, 1);
    assert_eq!(
        result.summary().shutdown_probability,
        ExactQty::from_integer(0)
    );
    close(result.summary().throughput.p10.value(), 160.0);
    close(result.summary().throughput.p90.value(), 160.0);

    // 120*(1/10-1/30)=8 and 800*(1/2)/8=50.
    let mut second = scenario("second", 12.0, 800.0, 0.5, one()).value;
    second.feed_size = Micrometres::new(900.0);
    let result = evaluate(&model(), &[scenario_claim(second)]).unwrap();
    close(result.points()[0].specific_energy.value(), 8.0);
    close(result.points()[0].throughput.value(), 50.0);
}

#[test]
fn joint_hardness_is_propagated_before_averaging_and_exact_quantiles_keep_ties() {
    let rows = [
        scenario("soft", 5.0, 1000.0, 0.8, half()),
        scenario("hard", 20.0, 1000.0, 0.8, half()),
    ];
    let result = evaluate(&model(), &rows).unwrap();
    // Authored values: Qsoft=320 and Qhard=80, so E[Q]=200.
    close(result.summary().specific_energy.mean.value(), 6.25);
    close(result.summary().throughput.mean.value(), 200.0);
    close(result.summary().throughput.p10.value(), 80.0);
    close(result.summary().throughput.p50.value(), 80.0);
    close(result.summary().throughput.p90.value(), 320.0);
    let at_mean = evaluate(&model(), &[scenario("mean", 12.5, 1000.0, 0.8, one())]).unwrap();
    close(at_mean.summary().throughput.mean.value(), 128.0);
    assert_ne!(
        result.summary().throughput.mean,
        at_mean.summary().throughput.mean
    );
    let mut reversed = rows.to_vec();
    reversed.reverse();
    let reordered = evaluate(&model(), &reversed).unwrap();
    assert_eq!(result, reordered);
}

#[test]
fn dependence_between_power_and_hardness_survives_identical_marginals() {
    // Both populations have Wi={5,20}, power={500,2000}, each with probability1/2.
    // Paired: {160,160}; opposite pairing: {640,40} =>340.
    let paired = evaluate(
        &model(),
        &[
            scenario("soft", 5.0, 500.0, 0.8, half()),
            scenario("hard", 20.0, 2000.0, 0.8, half()),
        ],
    )
    .unwrap();
    let opposite = evaluate(
        &model(),
        &[
            scenario("soft", 5.0, 2000.0, 0.8, half()),
            scenario("hard", 20.0, 500.0, 0.8, half()),
        ],
    )
    .unwrap();
    close(paired.summary().throughput.mean.value(), 160.0);
    close(opposite.summary().throughput.mean.value(), 340.0);
    assert_ne!(paired.digest(), opposite.digest());
}

#[test]
fn zero_power_or_utilization_is_honest_shutdown_with_positive_specific_energy() {
    for (power, u, expected) in [
        (0.0, 0.8, OperatingState::NoInstalledPower),
        (1000.0, 0.0, OperatingState::ZeroUtilization),
        (0.0, 0.0, OperatingState::NoPowerAndZeroUtilization),
    ] {
        let result = evaluate(&model(), &[scenario("shutdown", 10.0, power, u, one())]).unwrap();
        close(result.points()[0].specific_energy.value(), 5.0);
        assert_eq!(result.points()[0].throughput.value(), 0.0);
        assert_eq!(result.points()[0].effective_power.value(), 0.0);
        assert_eq!(result.points()[0].state, expected);
        assert_eq!(result.summary().shutdown_probability, one());
    }
    let mixed = evaluate(
        &model(),
        &[
            scenario("run", 10.0, 1000.0, 0.8, half()),
            scenario("stop", 10.0, 1000.0, 0.0, half()),
        ],
    )
    .unwrap();
    close(mixed.summary().throughput.mean.value(), 80.0);
    assert_eq!(mixed.summary().shutdown_probability, half());
}

#[test]
fn near_equal_sizes_do_not_cancel_and_energy_scales_with_grind_size() {
    let mut row = scenario("adjacent", 10.0, 1000.0, 0.8, one()).value;
    row.feed_size = Micrometres::new(100.0_f64.next_up());
    // Independently evaluated from the exact binary64 F value using Python
    // Decimal precision100 on the literal Bond expression, not this rearrangement.
    let result = evaluate(&model(), &[scenario_claim(row)]).unwrap();
    close(
        result.points()[0].specific_energy.value(),
        7.105_427_357_601_001e-16,
    );
    close(
        result.points()[0].throughput.value(),
        1.125_899_906_842_624_1e18,
    );

    let mut larger = scenario("larger", 10.0, 1000.0, 0.8, one()).value;
    larger.feed_size = Micrometres::new(1600.0);
    larger.product_size = Micrometres::new(400.0);
    let result = evaluate(&model(), &[scenario_claim(larger)]).unwrap();
    close(result.points()[0].specific_energy.value(), 2.5);
    close(result.points()[0].throughput.value(), 320.0);
}

#[test]
fn invalid_physics_and_declared_extrapolation_refuse_without_clamping() {
    for (wi, power, u) in [
        (0.0, 1000.0, 0.8),
        (-1.0, 1000.0, 0.8),
        (10.0, -1.0, 0.8),
        (10.0, 1000.0, -0.1),
        (10.0, 1000.0, 1.01),
        (f64::NAN, 1000.0, 0.8),
        (10.0, f64::INFINITY, 0.8),
    ] {
        assert_eq!(
            evaluate(&model(), &[scenario("bad", wi, power, u, one())])
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
    }
    for (f, p) in [
        (0.0, 100.0),
        (100.0, 0.0),
        (100.0, -1.0),
        (100.0, 100.0),
        (100.0, 101.0),
    ] {
        let mut row = scenario("bad_size", 10.0, 1000.0, 0.8, one()).value;
        row.feed_size = Micrometres::new(f);
        row.product_size = Micrometres::new(p);
        assert_eq!(
            evaluate(&model(), &[scenario_claim(row)]).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
    assert_eq!(
        evaluate(&model(), &[scenario("outside", 0.9, 1000.0, 0.8, one())])
            .unwrap_err()
            .code,
        "FMINE-REF-203"
    );
    let mut out_of_size = scenario("outside_size", 10.0, 1000.0, 0.8, one()).value;
    out_of_size.feed_size = Micrometres::new(2001.0);
    assert_eq!(
        evaluate(&model(), &[scenario_claim(out_of_size)])
            .unwrap_err()
            .code,
        "FMINE-REF-203"
    );
    let mut m = model().value;
    m.work_index_domain.swap(0, 1);
    assert_eq!(
        evaluate(
            &model_claim(m),
            &[scenario("nominal", 10.0, 1000.0, 0.8, one())]
        )
        .unwrap_err()
        .code,
        "FMINE-REF-109"
    );
}

#[test]
fn every_joint_member_must_be_complete_compatible_and_positive_weight() {
    let m = model();
    let row = scenario("a", 10.0, 1000.0, 0.8, half());
    assert_eq!(evaluate(&m, &[]).unwrap_err().code, "FMINE-REF-201");
    assert_eq!(
        evaluate(&m, &[row.clone()]).unwrap_err().code,
        "FMINE-REF-109"
    );
    assert_eq!(
        evaluate(&m, &[row.clone(), row]).unwrap_err().code,
        "FMINE-REF-105"
    );
    assert_eq!(
        evaluate(
            &m,
            &[scenario(
                "zero",
                10.0,
                1000.0,
                0.8,
                ExactQty::from_integer(0)
            )]
        )
        .unwrap_err()
        .code,
        "FMINE-REF-109"
    );
    let mut shifted = scenario("shifted", 10.0, 1000.0, 0.8, one()).value;
    shifted.scope.interval.end_ns += 1;
    assert_eq!(
        evaluate(&m, &[scenario_claim(shifted)]).unwrap_err().code,
        "FMINE-REF-102"
    );
    let mut false_complete = m.value;
    false_complete.complete_joint_population = false;
    assert_eq!(
        evaluate(
            &model_claim(false_complete),
            &[scenario("a", 10.0, 1000.0, 0.8, one())]
        )
        .unwrap_err()
        .code,
        "FMINE-REF-201"
    );
    let err = evaluate(
        &model(),
        &[
            scenario("valid", 10.0, 1000.0, 0.8, half()),
            scenario("invalid", 200.0, 1000.0, 0.8, half()),
        ],
    )
    .unwrap_err();
    assert_eq!(err.code, "FMINE-REF-203");
    assert!(err.what.contains("scenario invalid"));
}

#[test]
fn source_mutation_support_staleness_and_failed_calibration_are_not_admitted() {
    let original = scenario("nominal", 10.0, 1000.0, 0.8, one());
    let mut changed = original.clone();
    changed.value.work_index = KwhPerDryTonne::new(20.0);
    assert_eq!(
        evaluate(&model(), &[changed]).unwrap_err().code,
        "FMINE-REF-507"
    );
    let mut changed = original.clone();
    changed.support = SpatialSupport::PlantLot;
    assert_eq!(
        evaluate(&model(), &[changed]).unwrap_err().code,
        "FMINE-REF-102"
    );
    let mut changed = original.clone();
    changed.stale = true;
    assert_eq!(
        evaluate(&model(), &[changed]).unwrap_err().code,
        "FMINE-REF-502"
    );
    let mut changed = original;
    changed.calibration_failed = true;
    assert_eq!(
        evaluate(&model(), &[changed]).unwrap_err().code,
        "FMINE-REF-205"
    );
}

#[test]
fn output_remains_modelled_taint_and_unhashed_source_metadata_are_retained() {
    let m = model();
    let s = scenario("nominal", 10.0, 1000.0, 0.8, one());
    let original = evaluate(&m, std::slice::from_ref(&s)).unwrap();
    assert_eq!(
        original.claim().evidence,
        EvidenceClass::ModelledUncalibrated
    );
    assert_eq!(original.claim().state, KnowledgeState::Estimated);
    assert!(original.claim().parents.contains(&m.claim_id));
    assert!(original.claim().parents.contains(&s.claim_id));
    original.claim().check_id().unwrap();

    let mut with_uu = s.clone();
    with_uu.uu_terms.push(UuTerm {
        scenario_id: "unmodelled-hardness".into(),
        description: "synthetic unseen hardness".into(),
        downside_exposure_usd_m: 0.0,
        trigger_indicator: "new testwork outside declared range".into(),
    });
    let changed = evaluate(&m, &[with_uu]).unwrap();
    assert_ne!(changed.digest(), original.digest());
    assert_eq!(changed.claim().uu_terms.len(), 1);

    let labels = LabelRegistry::new([], ["W-synthetic-grinding"]).unwrap();
    let mut private = s;
    private.confidentiality = Confidentiality::mnpi(labels.wall("W-synthetic-grinding").unwrap());
    private.claim_id = fmine_core::id::ObjectId::from_canonical_bytes(
        fmine_core::id::PrefixKind::Claim,
        &private.canonical_body(),
    );
    let private_result = evaluate(&m, &[private]).unwrap();
    assert!(!private_result.claim().confidentiality.is_public());
    assert!(
        private_result
            .claim()
            .confidentiality
            .to_string()
            .contains("W-synthetic-grinding")
    );
}

#[test]
fn numerical_overflow_and_underflow_refuse_instead_of_zero_or_infinite_capacity() {
    let mut broad = model().value;
    broad.work_index_domain = [KwhPerDryTonne::new(1e-300), KwhPerDryTonne::new(1e300)];
    broad.feed_size_domain = [Micrometres::new(1e-300), Micrometres::new(1e308)];
    broad.product_size_domain = [Micrometres::new(1e-300), Micrometres::new(1e308)];
    let broad = model_claim(broad);
    for (wi, f, p) in [(1e300, 1e-299, 1e-300), (1e-300, 1e301, 1e300)] {
        let mut row = scenario("numerical", wi, 1000.0, 0.8, one()).value;
        row.feed_size = Micrometres::new(f);
        row.product_size = Micrometres::new(p);
        assert_eq!(
            evaluate(&broad, &[scenario_claim(row)]).unwrap_err().code,
            "FMINE-REF-207"
        );
    }
    assert_eq!(
        evaluate(
            &model(),
            &[scenario(
                "power_underflow",
                10.0,
                f64::from_bits(1),
                0.5,
                one()
            )]
        )
        .unwrap_err()
        .code,
        "FMINE-REF-207"
    );
}

#[test]
fn worksheet_preserves_exact_source_bytes_units_claims_and_joint_answers() {
    let nominal = worksheet(NOMINAL).unwrap();
    close(
        nominal.receipt().summary().specific_energy.mean.value(),
        5.0,
    );
    close(nominal.receipt().summary().throughput.mean.value(), 160.0);
    assert_eq!(nominal.receipt().claim().evidence, EvidenceClass::Assumed);
    assert_eq!(
        nominal.receipt().claim().confidentiality,
        Confidentiality::internal()
    );
    for c in nominal.receipt().scenarios() {
        assert_eq!(c.anchors[0].artifact_sha256, sha256(NOMINAL.as_bytes()).0);
        assert!(c.anchors[0].locator.contains("scenario row at line"));
    }
    let json = nominal.to_json();
    assert!(json.contains("\"schema\":\"fmine.plant-grinding-result.v1\""));
    assert!(json.contains("\"specific_energy_kwh_per_dry_t\":{\"mean\":5"));
    assert!(json.contains("\"throughput_dry_t_per_hour\":{\"mean\":160"));
    assert!(json.contains("\"calibration\":\"uncalibrated\""));
    assert!(nominal.to_markdown().contains("160"));
    assert!(
        nominal
            .to_markdown()
            .contains(&sha256(NOMINAL.as_bytes()).to_hex())
    );
    assert_eq!(nominal, worksheet(NOMINAL).unwrap());
    let joint = worksheet(JOINT).unwrap();
    close(joint.receipt().summary().throughput.mean.value(), 200.0);
    close(joint.receipt().summary().specific_energy.mean.value(), 6.25);
    let changed = format!("# Different exact source bytes\n{NOMINAL}");
    assert_ne!(nominal.digest(), worksheet(&changed).unwrap().digest());
    assert_eq!(
        worksheet::evaluate(&changed, &anchor(NOMINAL))
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
}

#[test]
fn worksheet_closed_grammar_refuses_missing_basis_unknown_fields_and_invalid_rows() {
    for (from, to, code) in [
        (
            "mass_basis = \"dry_metric_tonne\"",
            "mass_basis = \"wet_tonne\"",
            "FMINE-REF-105",
        ),
        (
            "size_unit = \"micron\"",
            "size_unit = \"mm\"",
            "FMINE-REF-105",
        ),
        (
            "power_unit = \"kW\"",
            "power_unit = \"MW\"",
            "FMINE-REF-105",
        ),
        ("weight = \"1\"", "weight = 1", "FMINE-REF-506"),
        ("weight = \"1\"", "weight = \"1e0\"", "FMINE-REF-506"),
        (
            "work_index = \"10\"",
            "work_index = \"NaN\"",
            "FMINE-REF-506",
        ),
        (
            "work_index = \"10\"",
            "work_index = \"1e-999\"",
            "FMINE-REF-207",
        ),
        (
            "work_index = \"10\"",
            "work_index = \"21\"",
            "FMINE-REF-203",
        ),
        (
            "p80_microns = \"100\"",
            "p80_microns = \"400\"",
            "FMINE-REF-109",
        ),
        (
            "complete_joint_population = \"true\"",
            "complete_joint_population = \"false\"",
            "FMINE-REF-201",
        ),
        ("energy_unit = \"kWh\"\n", "", "FMINE-REF-506"),
        ("[[scenario]]", "[[typo]]", "FMINE-REF-506"),
    ] {
        let changed = NOMINAL.replace(from, to);
        assert_ne!(changed, NOMINAL, "test mutation must apply");
        assert_eq!(
            worksheet(&changed).unwrap_err().code,
            code,
            "{from} -> {to}"
        );
    }
    for added in ["\nunknown = \"1\"\n", "\nweight = \"1\"\n"] {
        assert_eq!(
            worksheet(&format!("{NOMINAL}{added}")).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
}
