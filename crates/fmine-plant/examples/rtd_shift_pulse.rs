//! Synthetic GB-41 pulse through eight equal tanks with a 24 h mean residence.
//! The feed is one dry metric tonne per interval at 3 g/t Au, except 9 g/t first.
//! Independent Erlang-integral tail observations assume 88% recovery AFTER delay.
//! This illustrates a declared hypothesis; it is no fitted or measured plant.
//!
//! cargo run -p fmine-plant --example rtd_shift_pulse
//! --slow-shifts doubles alternate durations at unchanged integrated throughput.
//! --outside-flow supplies an unsupported interval-mean flow and refuses before stdout.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_plant::{
    flowsheet::{ElementKey, FlowsheetScope, MaterialAmounts},
    reconciliation::Kilograms,
    rtd::{
        CubicMetres, CubicMetresPerSecond, FEED_METRIC, MODEL_METRIC, MaterialChannel,
        OPENING_METRIC, RtdFeed, RtdModel, RtdOpening, propagate,
        recovery::{
            DECLARATION_METRIC, RecoveryDeclaration, TAIL_METRIC, TailObservation, compare_recovery,
        },
    },
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

const HOUR: i64 = 3_600_000_000_000;
const BOUNDARY: &str = "synthetic-GB41-passive-delay/v1";
// Independent 75-digit Decimal evaluation, rounded into exact source literals:
// G(x)=x-8+e^-x*sum_(j=0..7)(8-j)x^j/j!;
// mean grade(k)=3+6*[G(4k+4)-2G(4k)+G(4k-4)]/4, with G(x<=0)=0.
// Tail kilograms=.12*mean grade/1000 for each 1 dry metric tonne interval.
const TAILS: [&str; 4] = [
    "0.000366052857708152889",
    "0.000548898890592553796",
    "0.000713953938260800525",
    "0.000504046296833828816",
];

fn exact(value: &str) -> Result<ExactQty, Refusal> {
    ExactQty::from_decimal_str(value)
        .map_err(|e| Refusal::new(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}
fn au() -> ElementKey {
    ElementKey {
        element: "Au".into(),
        measurand: "total-Au/all-phases".into(),
    }
}
fn amounts(dry: &str, water: &str, gold: &str) -> Result<MaterialAmounts, Refusal> {
    Ok(MaterialAmounts {
        dry_solids_kg: exact(dry)?,
        water_kg: exact(water)?,
        elements_kg: BTreeMap::from([(au(), exact(gold)?)]),
    })
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
            "crates/fmine-plant/examples/rtd_shift_pulse.rs",
            sha256(include_bytes!("rtd_shift_pulse.rs")).0,
            format!("constructed equal-tank example {tag}"),
        )],
    )
}
fn main() -> Result<(), Refusal> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() > 1
        || args
            .iter()
            .any(|arg| arg != "--slow-shifts" && arg != "--outside-flow")
    {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "use no argument, --slow-shifts or --outside-flow",
        ));
    }
    let slow = args.iter().any(|arg| arg == "--slow-shifts");
    let outside = args.iter().any(|arg| arg == "--outside-flow");
    let durations = [
        if outside { 1 } else { 12 },
        if slow { 24 } else { 12 },
        12,
        if slow { 24 } else { 12 },
    ];
    let scope = FlowsheetScope {
        boundary: BOUNDARY.into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: durations.iter().sum::<i64>() * HOUR,
        },
    };
    let model = RtdModel {
        scope: scope.clone(),
        elements: BTreeSet::from([au()]),
        tanks: 8,
        volume_m3: CubicMetres::new(24.0),
        bypass_fraction: 0.0,
        dead_fraction: 0.0,
        total_flow_domain_m3_per_s: [
            CubicMetresPerSecond::new(0.0),
            CubicMetresPerSecond::new(0.0004),
        ],
        common_conservative_transport: true,
        piecewise_constant_feed_per_volume: true,
        closure_tolerances_kg: BTreeMap::from([
            (MaterialChannel::DrySolids, Kilograms::new(1e-8)),
            (MaterialChannel::Water, Kilograms::new(1e-8)),
            (MaterialChannel::Element(au()), Kilograms::new(1e-12)),
        ]),
        max_kernel_steps: 64,
    };
    let tag = model.canonical_tag();
    let model = source(MODEL_METRIC, model, tag, AdditivityClass::NonAdditive);
    let opening = RtdOpening {
        boundary: BOUNDARY.into(),
        at_ns: 0,
        tanks: vec![amounts("250", "500", "0.00075")?; 8],
        dead: amounts("0", "0", "0")?,
    };
    let tag = opening.canonical_tag();
    let opening = source(
        OPENING_METRIC,
        opening,
        tag,
        AdditivityClass::ExtensiveAdditive,
    );
    let mut feeds = Vec::new();
    let mut tails = Vec::new();
    let mut start_ns = 0;
    for (index, hours) in durations.into_iter().enumerate() {
        let interval = ValidTime {
            start_ns,
            end_ns: start_ns + hours * HOUR,
        };
        let period = FlowsheetScope {
            boundary: BOUNDARY.into(),
            interval,
        };
        let feed = RtdFeed {
            scope: period.clone(),
            slurry_volume_m3: CubicMetres::new(12.0),
            amounts: amounts("1000", "2000", if index == 0 { "0.009" } else { "0.003" })?,
        };
        let tag = feed.canonical_tag();
        feeds.push(source(
            FEED_METRIC,
            feed,
            tag,
            AdditivityClass::ExtensiveAdditive,
        ));
        let tail = TailObservation {
            scope: period,
            element: au(),
            tail_element_kg: exact(TAILS[index])?,
        };
        let tag = tail.canonical_tag();
        tails.push(source(
            TAIL_METRIC,
            tail,
            tag,
            AdditivityClass::ExtensiveAdditive,
        ));
        start_ns = interval.end_ns;
    }
    let transport = propagate(&model, &opening, &feeds, || false)?;
    let declaration = RecoveryDeclaration {
        transport_claim: transport.claim().claim_id,
        element: au(),
        delay_then_separation: true,
        complete_tail_population: true,
        all_non_tail_element_is_recovered: true,
    };
    let tag = declaration.canonical_tag();
    let declaration = source(
        DECLARATION_METRIC,
        declaration,
        tag,
        AdditivityClass::NonAdditive,
    );
    let recovery = compare_recovery(&transport, &declaration, &tails)?;
    // No successful output until every source, interval and comparison is admitted.
    println!(
        "SYNTHETIC known RTD and delay-then-separation hypothesis; no fitted recovery or plant calibration."
    );
    println!(
        "hours | feed Au kg | aligned Au kg | tail Au kg | naive recovery | aligned recovery | Au stock kg"
    );
    for (comparison, interval) in recovery
        .claim()
        .value
        .intervals
        .iter()
        .zip(&transport.summary().intervals)
    {
        let naive = comparison
            .same_interval_recovery
            .map_or("undefined".into(), |r| format!("{:.9}%", 100.0 * r));
        let aligned = comparison
            .lag_aligned_recovery
            .map_or("undefined".into(), |r| format!("{:.9}%", 100.0 * r));
        println!(
            "{}..{} | {:.12} | {:.12} | {:.12} | {} | {} | {:.12}",
            comparison.interval.start_ns / HOUR,
            comparison.interval.end_ns / HOUR,
            comparison.same_interval_feed_element_kg.value(),
            comparison.aligned_feed_equivalent_kg.value(),
            comparison.tail_element_kg.value(),
            naive,
            aligned,
            interval.closing_inventory_kg[&MaterialChannel::Element(au())].value()
        );
    }
    let residuals = &transport.summary().whole_window_residual_kg;
    println!(
        "Passive feed-equivalent residuals (kg): dry={:.6e}, water={:.6e}, Au={:.6e}",
        residuals[&MaterialChannel::DrySolids].value(),
        residuals[&MaterialChannel::Water].value(),
        residuals[&MaterialChannel::Element(au())].value()
    );
    println!("Source-bound transport receipt: {}", transport.digest());
    println!("Conditional recovery receipt: {}", recovery.digest());
    Ok(())
}
