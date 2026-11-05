//! Independent equal-tank closed forms, GB-41 pulse and throughput-volume checks.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, LabelRegistry, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use recovery::{RecoveryDeclaration, TailObservation, compare_recovery};

const SECOND: i64 = 1_000_000_000;
const HOUR: i64 = 3600 * SECOND;
const BOUNDARY: &str = "synthetic-equal-tank-train/v1";

fn q(value: &str) -> ExactQty {
    ExactQty::from_decimal_str(value).unwrap()
}
fn au() -> ElementKey {
    ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    }
}
fn gold() -> MaterialChannel {
    MaterialChannel::Element(au())
}
fn amounts(dry: &str, water: &str, metal: &str) -> MaterialAmounts {
    MaterialAmounts {
        dry_solids_kg: q(dry),
        water_kg: q(water),
        elements_kg: BTreeMap::from([(au(), q(metal))]),
    }
}
fn scope(start_ns: i64, end_ns: i64) -> FlowsheetScope {
    FlowsheetScope {
        boundary: BOUNDARY.into(),
        interval: ValidTime { start_ns, end_ns },
    }
}
fn source<T>(metric: &str, value: T, tag: String, additivity: AdditivityClass) -> Claim<T> {
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "crates/fmine-plant/src/rtd/tests.rs",
            sha256(include_bytes!("tests.rs")).0,
            format!("constructed source {tag}"),
        )],
    )
}
fn model_claim(value: RtdModel) -> Claim<RtdModel> {
    let tag = value.canonical_tag();
    source(MODEL_METRIC, value, tag, AdditivityClass::NonAdditive)
}
fn opening_claim(value: RtdOpening) -> Claim<RtdOpening> {
    let tag = value.canonical_tag();
    source(
        OPENING_METRIC,
        value,
        tag,
        AdditivityClass::ExtensiveAdditive,
    )
}
fn feed_claim(value: RtdFeed) -> Claim<RtdFeed> {
    let tag = value.canonical_tag();
    source(FEED_METRIC, value, tag, AdditivityClass::ExtensiveAdditive)
}
fn model(n: usize, volume: f64, end: i64) -> Claim<RtdModel> {
    model_claim(RtdModel {
        scope: scope(0, end),
        elements: BTreeSet::from([au()]),
        tanks: n,
        volume_m3: CubicMetres::new(volume),
        bypass_fraction: 0.0,
        dead_fraction: 0.0,
        total_flow_domain_m3_per_s: [
            CubicMetresPerSecond::new(0.0),
            CubicMetresPerSecond::new(1000.0),
        ],
        common_conservative_transport: true,
        piecewise_constant_feed_per_volume: true,
        closure_tolerances_kg: BTreeMap::from([
            (MaterialChannel::DrySolids, Kilograms::new(1e-8)),
            (MaterialChannel::Water, Kilograms::new(1e-8)),
            (gold(), Kilograms::new(1e-12)),
        ]),
        max_kernel_steps: 1024,
    })
}
fn opening(tanks: Vec<MaterialAmounts>) -> Claim<RtdOpening> {
    opening_claim(RtdOpening {
        boundary: BOUNDARY.into(),
        at_ns: 0,
        tanks,
        dead: amounts("0", "0", "0"),
    })
}
fn feed(start: i64, end: i64, volume: f64, material: MaterialAmounts) -> Claim<RtdFeed> {
    feed_claim(RtdFeed {
        scope: scope(start, end),
        slurry_volume_m3: CubicMetres::new(volume),
        amounts: material,
    })
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:.17e} expected={expected:.17e}"
    );
}
fn kg(values: &ChannelMasses, channel: MaterialChannel) -> f64 {
    values[&channel].value()
}
fn declaration(receipt: &RtdReceipt) -> Claim<RecoveryDeclaration> {
    let value = RecoveryDeclaration {
        transport_claim: receipt.claim().claim_id,
        element: au(),
        delay_then_separation: true,
        complete_tail_population: true,
        all_non_tail_element_is_recovered: true,
    };
    let tag = value.canonical_tag();
    source(
        recovery::DECLARATION_METRIC,
        value,
        tag,
        AdditivityClass::NonAdditive,
    )
}
fn tail(period: FlowsheetScope, amount: &str) -> Claim<TailObservation> {
    let value = TailObservation {
        scope: period,
        element: au(),
        tail_element_kg: q(amount),
    };
    let tag = value.canonical_tag();
    source(
        recovery::TAIL_METRIC,
        value,
        tag,
        AdditivityClass::ExtensiveAdditive,
    )
}

#[test]
fn one_and_two_tank_closed_forms_integrate_output_independently() {
    // One tank x=1: M(1)=F+(M0-F)e^-1; integral outlet=F+(M0-F)(1-e^-1).
    let m = model(1, 1.0, SECOND);
    let o = opening(vec![amounts("10", "20", "1")]);
    let f = vec![feed(0, SECOND, 1.0, amounts("20", "40", "2"))];
    let r = propagate(&m, &o, &f, || false).unwrap();
    let e = (-1.0_f64).exp();
    close(kg(&r.summary().final_tanks_kg[0], gold()), 2.0 - e, 3e-15);
    close(
        kg(&r.summary().intervals[0].total_outflow_kg, gold()),
        1.0 + e,
        3e-15,
    );
    close(
        kg(&r.summary().final_tanks_kg[0], MaterialChannel::Water),
        40.0 - 20.0 * e,
        5e-14,
    );
    // Two tanks x=1, F=2: M1=2-e^-1; M2=2+e^-1[(3-2)+(1-2)]=2.
    // Integrating the independent c2(t)=2+(1-t)e^-t gives outlet=2+e^-1.
    let m = model(2, 2.0, SECOND);
    let o = opening(vec![amounts("1", "2", "0.1"), amounts("3", "6", "0.3")]);
    let f = vec![feed(0, SECOND, 1.0, amounts("2", "4", "0.2"))];
    let r = propagate(&m, &o, &f, || false).unwrap();
    close(
        kg(&r.summary().final_tanks_kg[0], MaterialChannel::DrySolids),
        2.0 - e,
        2e-15,
    );
    close(
        kg(&r.summary().final_tanks_kg[1], MaterialChannel::DrySolids),
        2.0,
        2e-15,
    );
    close(
        kg(
            &r.summary().intervals[0].total_outflow_kg,
            MaterialChannel::DrySolids,
        ),
        2.0 + e,
        2e-15,
    );
    for residual in r.summary().whole_window_residual_kg.values() {
        assert!(residual.value().abs() < 1e-14);
    }
}

// Independent 75-digit Decimal evaluation of the N=8 Erlang integral:
// G(x)=x-8+e^-x*sum_(j=0..7)[(8-j)x^j/j!]. For the 12 h rectangular pulse,
// mean grade in shift k=3+6*[G(4(k+1))-2G(4k)+G(4(k-1))]/4; G(x<=0)=0.
// Tail metal for a one-tonne shift is .12*grade/1000 kg. These literals are not
// generated from the candidate's transition or its returned inventory balance.
const GB41_TAIL_KG: [&str; 4] = [
    "0.000366052857708152889",
    "0.000548898890592553796",
    "0.000713953938260800525",
    "0.000504046296833828816",
];

#[test]
fn gb41_rectangular_grade_pulse_recovers_flat_88_percent_from_independent_tails() {
    let m = model(8, 24.0, 48 * HOUR);
    let o = opening(vec![amounts("250", "500", "0.00075"); 8]);
    let f: Vec<_> = (0..4)
        .map(|k| {
            feed(
                k * 12 * HOUR,
                (k + 1) * 12 * HOUR,
                12.0,
                amounts("1000", "2000", if k == 0 { "0.009" } else { "0.003" }),
            )
        })
        .collect();
    let r = propagate(&m, &o, &f, || false).unwrap();
    let tails: Vec<_> = f
        .iter()
        .zip(GB41_TAIL_KG)
        .map(|(f, amount)| tail(f.value.scope.clone(), amount))
        .collect();
    let d = declaration(&r);
    let c = compare_recovery(&r, &d, &tails).unwrap();
    let naive = [
        0.9593274602546497,
        0.8170337031358154,
        0.7620153539130665,
        0.8319845677220571,
    ];
    for (point, wrong) in c.claim().value.intervals.iter().zip(naive) {
        close(point.lag_aligned_recovery.unwrap(), 0.88, 4e-15);
        close(point.same_interval_recovery.unwrap(), wrong, 4e-15);
    }
    assert_eq!(c.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(c.claim().state, KnowledgeState::Estimated);
    assert_eq!(c.observations(), tails);
    assert_eq!(r.feeds(), f);
    assert_eq!(r.opening(), &o);
    assert_eq!(r.model(), &m);
}

#[test]
fn varying_clock_duration_uses_actual_throughput_volume_and_retains_shutdown_stock() {
    let o = opening(vec![amounts("10", "20", "1"); 2]);
    let m = model(2, 2.0, 3 * SECOND);
    let regular = vec![
        feed(0, SECOND, 1.0, amounts("20", "40", "2")),
        feed(SECOND, 2 * SECOND, 1.0, amounts("10", "20", "1")),
        feed(2 * SECOND, 3 * SECOND, 1.0, amounts("10", "20", "1")),
    ];
    let a = propagate(&m, &o, &regular, || false).unwrap();
    let slow = vec![
        feed(0, 2 * SECOND, 1.0, amounts("20", "40", "2")),
        feed(2 * SECOND, 4 * SECOND, 0.0, amounts("0", "0", "0")),
        feed(4 * SECOND, 7 * SECOND, 1.0, amounts("10", "20", "1")),
        feed(7 * SECOND, 8 * SECOND, 1.0, amounts("10", "20", "1")),
    ];
    let b = propagate(&model(2, 2.0, 8 * SECOND), &o, &slow, || false).unwrap();
    assert_eq!(a.summary().final_tanks_kg, b.summary().final_tanks_kg);
    for (a, index) in a.summary().intervals.iter().zip([0, 2, 3]) {
        assert_eq!(
            a.total_outflow_kg,
            b.summary().intervals[index].total_outflow_kg
        );
    }
    let stopped = &b.summary().intervals[1];
    assert_eq!(stopped.opening_inventory_kg, stopped.closing_inventory_kg);
    assert!(stopped.total_outflow_kg.values().all(|q| q.value() == 0.0));
}

#[test]
fn bypass_reduces_actual_tank_flow_and_dead_inventory_never_disappears() {
    let mut m = model(1, 2.0, SECOND).value;
    m.bypass_fraction = 0.5;
    m.dead_fraction = 0.5;
    let m = model_claim(m);
    let mut o = opening(vec![amounts("10", "20", "1")]).value;
    o.dead = amounts("7", "14", "0.7");
    let o = opening_claim(o);
    let f = vec![feed(0, SECOND, 2.0, amounts("40", "80", "4"))];
    let r = propagate(&m, &o, &f, || false).unwrap();
    let point = &r.summary().intervals[0];
    // V_active=1 m3, Q_active*dt=.5*2=1 m3; x=1, NOT2.
    assert_eq!(point.active_tank_turnovers, 1.0);
    close(kg(&point.bypass_kg, gold()), 2.0, 0.0);
    close(
        kg(&point.tank_outflow_kg, gold()),
        1.0 + (-1.0_f64).exp(),
        2e-15,
    );
    close(kg(&r.summary().dead_inventory_kg, gold()), 0.7, 0.0);
    close(
        kg(&point.closing_inventory_kg, gold()),
        2.7 - (-1.0_f64).exp(),
        2e-15,
    );
    let mut bypass_only = m.value.clone();
    bypass_only.bypass_fraction = 1.0;
    let identity = propagate(&model_claim(bypass_only), &o, &f, || false).unwrap();
    let p = &identity.summary().intervals[0];
    assert_eq!(p.feed_kg, p.total_outflow_kg);
    assert_eq!(p.opening_inventory_kg, p.closing_inventory_kg);
    assert_eq!(identity.summary().kernel_steps, 0);
}

#[test]
fn long_interval_composition_matches_split_intervals_without_time_discretization_fit() {
    let o = opening(vec![amounts("1", "2", "0.1"); 8]);
    let m = model(8, 8.0, 4 * SECOND);
    let one = vec![feed(0, 4 * SECOND, 120.0, amounts("240", "480", "24"))];
    let whole = propagate(&m, &o, &one, || false).unwrap();
    let four: Vec<_> = (0..4)
        .map(|k| {
            feed(
                k * SECOND,
                (k + 1) * SECOND,
                30.0,
                amounts("60", "120", "6"),
            )
        })
        .collect();
    let split = propagate(&m, &o, &four, || false).unwrap();
    assert_eq!(whole.summary().kernel_steps, 4);
    assert_eq!(
        whole.summary().final_tanks_kg,
        split.summary().final_tanks_kg
    );
    for channel in channel_order(&m.value) {
        close(
            whole.summary().intervals[0].total_outflow_kg[&channel].value(),
            split
                .summary()
                .intervals
                .iter()
                .map(|p| p.total_outflow_kg[&channel].value())
                .sum(),
            1e-12,
        );
    }
}

#[test]
fn support_domain_population_and_source_mutations_refuse_without_a_partial_receipt() {
    let m = model(1, 1.0, SECOND);
    let o = opening(vec![amounts("1", "2", "0.1")]);
    let f = vec![feed(0, SECOND, 1.0, amounts("2", "4", "0.2"))];
    let mut changed = f.clone();
    changed[0].value.amounts.elements_kg.insert(au(), q("0.3"));
    assert_eq!(
        propagate(&m, &o, &changed, || false).unwrap_err().code,
        "FMINE-REF-507"
    );
    let missing = opening(vec![]);
    assert_eq!(
        propagate(&m, &missing, &f, || false).unwrap_err().code,
        "FMINE-REF-102"
    );
    let mut outside = m.value.clone();
    outside.total_flow_domain_m3_per_s[1] = CubicMetresPerSecond::new(0.5);
    assert_eq!(
        propagate(&model_claim(outside), &o, &f, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-203"
    );
    let mut undeclared = m.value.clone();
    undeclared.common_conservative_transport = false;
    assert_eq!(
        propagate(&model_claim(undeclared), &o, &f, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-201"
    );
    let wrong = vec![feed(1, SECOND, 1.0, amounts("2", "4", "0.2"))];
    assert_eq!(
        propagate(&m, &o, &wrong, || false).unwrap_err().code,
        "FMINE-REF-102"
    );
    let no_flow = vec![feed(0, SECOND, 0.0, amounts("2", "4", "0.2"))];
    assert_eq!(
        propagate(&m, &o, &no_flow, || false).unwrap_err().code,
        "FMINE-REF-109"
    );
    let impossible = vec![feed(0, SECOND, 1.0, amounts("1", "4", "2"))];
    assert_eq!(
        propagate(&m, &o, &impossible, || false).unwrap_err().code,
        "FMINE-REF-109"
    );
    let same = f.clone();
    let mut calls = 0;
    assert_eq!(
        propagate(&m, &o, &f, || {
            calls += 1;
            calls == 3
        })
        .unwrap_err()
        .code,
        "FMINE-REF-201"
    );
    assert_eq!(f, same);
    let mut budget = m.value.clone();
    budget.max_kernel_steps = 1;
    let far = vec![feed(0, SECOND, 64.0, amounts("2", "4", "0.2"))];
    assert_eq!(
        propagate(&model_claim(budget), &o, &far, || false)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
}

#[test]
fn source_precision_evidence_and_order_are_retained() {
    let mut m = model(1, 1.0, 2 * SECOND);
    let registry = LabelRegistry::new(["JV-A"], ["W-A"]).unwrap();
    let wall = Confidentiality::mnpi(registry.wall("W-A").unwrap());
    m = Claim::new(
        MODEL_METRIC,
        m.value.clone(),
        &m.value.canonical_tag(),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        wall.clone(),
        vec![],
        vec![],
        m.anchors.clone(),
    );
    let o = opening(vec![amounts("10", "20", "1")]);
    let mut feeds = vec![
        feed(0, SECOND, 1.0, amounts("10", "20", "1")),
        feed(SECOND, 2 * SECOND, 1.0, amounts("10", "20", "1")),
    ];
    let a = propagate(&m, &o, &feeds, || false).unwrap();
    feeds.reverse();
    let b = propagate(&m, &o, &feeds, || false).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.claim().confidentiality, wall);
    assert_eq!(
        a.claim()
            .verify_export_clearance(&Confidentiality::public())
            .unwrap_err()
            .code,
        "FMINE-REF-301"
    );
    assert_eq!(a.claim().parents.len(), 4);
    assert_eq!(a.claim().state, KnowledgeState::Estimated);
    let big = vec![feed(
        0,
        2 * SECOND,
        1.0,
        amounts("10000000000000001", "0", "1"),
    )];
    assert_eq!(
        propagate(&m, &o, &big, || false).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn recovery_requires_its_separation_hypothesis_and_preserves_zero_denominators() {
    let m = model(1, 1.0, SECOND);
    let o = opening(vec![amounts("0", "0", "0")]);
    let f = vec![feed(0, SECOND, 0.0, amounts("0", "0", "0"))];
    let r = propagate(&m, &o, &f, || false).unwrap();
    let d = declaration(&r);
    let tails = vec![tail(scope(0, SECOND), "0")];
    let c = compare_recovery(&r, &d, &tails).unwrap();
    assert_eq!(c.claim().value.intervals[0].same_interval_recovery, None);
    assert_eq!(c.claim().value.intervals[0].lag_aligned_recovery, None);
    let mut unbound = d.value.clone();
    unbound.delay_then_separation = false;
    let tag = unbound.canonical_tag();
    let unbound = source(
        recovery::DECLARATION_METRIC,
        unbound,
        tag,
        AdditivityClass::NonAdditive,
    );
    assert_eq!(
        compare_recovery(&r, &unbound, &tails).unwrap_err().code,
        "FMINE-REF-201"
    );
    assert_eq!(
        compare_recovery(&r, &d, &[]).unwrap_err().code,
        "FMINE-REF-102"
    );
    assert_eq!(
        compare_recovery(&r, &d, &[tail(scope(0, SECOND), "0.001")])
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
}
