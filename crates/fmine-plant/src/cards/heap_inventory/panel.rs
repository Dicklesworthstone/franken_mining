//! DESIGN (BD-183, plan §20.8): matched panel solution flow and grade update joint lift scenarios.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;

use fmine_claim::{AdditivityClass, Claim};
use fmine_core::{codec::Encoder, det, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::ExactQty;

use super::{
    GaussianObservation, HeapInventoryEstimate, HeapInventoryInput, NS_PER_DAY, Result, arithmetic,
    estimate_population, invalid, inventory, is_fraction, named, put_exact, validate_population,
};

pub const INPUT_METRIC: &str = "plant.heap.panel_inventory_inputs/1";
const MAX_PANELS: usize = 128;
const MAX_APPLICATION_PERIODS: usize = 32;

/// One matched interval of pregnant and entering barren solution on a panel.
/// Grades are flow-weighted metal concentrations over the respective volumes.
/// The caller declares a background solution inventory with no net change
/// apart from the modeled transport. The Gaussian SD is on the net metal kg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelSolutionObservation {
    pub start_ns: i64,
    pub end_ns: i64,
    pub available_at_ns: i64,
    pub pregnant_litres: ExactQty,
    pub pregnant_mg_per_litre: ExactQty,
    pub barren_litres: ExactQty,
    pub barren_mg_per_litre: ExactQty,
    pub standard_deviation_kg: ExactQty,
}

/// The rate applies from irrigation start or the previous period's end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationPeriod {
    pub end_ns: i64,
    pub litres_per_m2_hour: ExactQty,
}

/// A disjoint partition of a lift. Application periods cover continuous
/// positive irrigation through the cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapPanel {
    pub lift: String,
    pub contained_kg: ExactQty,
    pub drainage_depth_m: ExactQty,
    pub application: Vec<ApplicationPeriod>,
    pub observation: Option<PanelSolutionObservation>,
}

/// Transport assumptions for one panel in one joint scenario. Channelling
/// sends a fraction of mobilized metal via a fast path. The remainder has a
/// piston-flow delay determined by the applied water depth since mobilization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelHydraulics {
    pub mobile_water_fraction: ExactQty,
    pub channelled_fraction: ExactQty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelHeapInventoryInput {
    pub heap: HeapInventoryInput,
    pub panels: BTreeMap<String, HeapPanel>,
    /// Joint scenario ID → panel ID → declared transport coordinates.
    pub hydraulics: BTreeMap<String, BTreeMap<String, PanelHydraulics>>,
}

impl PanelHeapInventoryInput {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"HPPL", 1);
        enc.put_str(&self.heap.canonical_tag());
        enc.put_u64(self.panels.len() as u64);
        for (id, panel) in &self.panels {
            enc.put_str(id);
            enc.put_str(&panel.lift);
            put_exact(&mut enc, panel.contained_kg);
            put_exact(&mut enc, panel.drainage_depth_m);
            enc.put_u64(panel.application.len() as u64);
            for period in &panel.application {
                enc.put_i64(period.end_ns);
                put_exact(&mut enc, period.litres_per_m2_hour);
            }
            enc.put_u8(u8::from(panel.observation.is_some()));
            if let Some(obs) = &panel.observation {
                enc.put_i64(obs.start_ns);
                enc.put_i64(obs.end_ns);
                enc.put_i64(obs.available_at_ns);
                for q in [
                    obs.pregnant_litres,
                    obs.pregnant_mg_per_litre,
                    obs.barren_litres,
                    obs.barren_mg_per_litre,
                    obs.standard_deviation_kg,
                ] {
                    put_exact(&mut enc, q);
                }
            }
        }
        enc.put_u64(self.hydraulics.len() as u64);
        for (scenario, panels) in &self.hydraulics {
            enc.put_str(scenario);
            enc.put_u64(panels.len() as u64);
            for (id, hydraulics) in panels {
                enc.put_str(id);
                put_exact(&mut enc, hydraulics.mobile_water_fraction);
                put_exact(&mut enc, hydraulics.channelled_fraction);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// The increment of a first-order CDF on an interval in mobilization time.
fn collected_fraction(start_days: f64, duration_days: f64, rate: f64) -> f64 {
    det::exp(-rate * start_days) * -det::expm1(-rate * duration_days)
}

struct FluxWindow {
    start_ns: i64,
    end_ns: i64,
    metres_per_day: f64,
}

fn flux_windows(
    id: &str,
    panel: &HeapPanel,
    irrigated_at_ns: i64,
    cut_ns: i64,
) -> Result<Vec<FluxWindow>> {
    let mut start_ns = irrigated_at_ns;
    let mut windows = Vec::with_capacity(panel.application.len());
    for period in &panel.application {
        if period.end_ns <= start_ns
            || period.end_ns > cut_ns
            || period.litres_per_m2_hour.signum() <= 0
        {
            return Err(invalid(format!(
                "panel {id} requires positive ordered application periods within the cut"
            )));
        }
        windows.push(FluxWindow {
            start_ns,
            end_ns: period.end_ns,
            metres_per_day: period.litres_per_m2_hour.to_f64() * 0.024,
        });
        start_ns = period.end_ns;
    }
    if start_ns != cut_ns {
        return Err(invalid(format!(
            "panel {id} application periods do not cover its active history"
        )));
    }
    Ok(windows)
}

/// Walk backward from collection until the matching pore-water depth has
/// passed. A collection time before the first slow arrival maps to age zero.
fn mobilization_age(
    windows: &[FluxWindow],
    irrigated_at_ns: i64,
    collected_at_ns: i64,
    mut depth_m: f64,
) -> f64 {
    for window in windows.iter().rev() {
        let end_ns = collected_at_ns.min(window.end_ns);
        if end_ns <= window.start_ns {
            continue;
        }
        let elapsed = age_days(end_ns, window.start_ns);
        let applied = elapsed * window.metres_per_day;
        if depth_m <= applied {
            return age_days(window.start_ns, irrigated_at_ns)
                + (applied - depth_m) / window.metres_per_day;
        }
        depth_m -= applied;
    }
    0.0
}

struct MeasuredPanel<'a> {
    id: &'a str,
    panel: &'a HeapPanel,
    irrigated_at_ns: i64,
    observation: &'a PanelSolutionObservation,
    windows: Vec<FluxWindow>,
}

fn net_metal_kg(obs: &PanelSolutionObservation) -> Result<ExactQty> {
    let scale = ExactQty::from_integer(1_000_000);
    let pregnant = obs
        .pregnant_litres
        .checked_mul(obs.pregnant_mg_per_litre)
        .map_err(arithmetic)?;
    let barren = obs
        .barren_litres
        .checked_mul(obs.barren_mg_per_litre)
        .map_err(arithmetic)?;
    pregnant
        .checked_sub(barren)
        .and_then(|net| net.checked_div(scale))
        .map_err(arithmetic)
}

fn age_days(end_ns: i64, start_ns: i64) -> f64 {
    (i128::from(end_ns) - i128::from(start_ns)) as f64 / NS_PER_DAY
}

/// Update recoverable inventory using net collected metal on named panels.
/// Panel measurements affect likelihoods. Recorded lift recoveries and losses
/// supply the inventory debits. The model assumes independent measurement
/// errors across the admitted observations and piecewise-constant application
/// rates under fixed transport fractions and depth.
pub fn estimate_heap_inventory_from_panels(
    input: &Claim<PanelHeapInventoryInput>,
) -> Result<Claim<HeapInventoryEstimate>> {
    let value = &input.value;
    validate_population(&value.heap)?;
    if !(1..=MAX_PANELS).contains(&value.panels.len())
        || value
            .panels
            .values()
            .any(|panel| panel.application.len() > MAX_APPLICATION_PERIODS)
        || !value.hydraulics.keys().eq(value.heap.scenarios.keys())
        || value
            .hydraulics
            .values()
            .any(|panels| !panels.keys().eq(value.panels.keys()))
    {
        return Err(invalid(
            "panel population and hydraulics must match every declared scenario",
        ));
    }
    inventory::admit_source(
        input,
        INPUT_METRIC,
        &value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    for (scenario, panels) in &value.hydraulics {
        for (id, h) in panels {
            if !is_fraction(h.mobile_water_fraction) || !is_fraction(h.channelled_fraction) {
                return Err(invalid(format!(
                    "invalid hydraulic fractions for scenario {scenario}, panel {id}"
                )));
            }
        }
    }

    let zero = ExactQty::from_integer(0);
    let mut lift_totals = BTreeMap::new();
    let mut measured = Vec::new();
    let mut evidence = Vec::new();
    for (id, panel) in &value.panels {
        let lift = value
            .heap
            .lifts
            .get(&panel.lift)
            .ok_or_else(|| invalid(format!("panel {id} names an unknown lift")))?;
        if !named(id) || panel.contained_kg.signum() <= 0 || panel.drainage_depth_m.signum() < 0 {
            return Err(invalid(format!(
                "invalid net-solution model for panel {id}"
            )));
        }
        let windows = flux_windows(id, panel, lift.irrigated_at_ns, value.heap.as_of_ns)?;
        let subtotal = lift_totals.entry(&panel.lift).or_insert(zero);
        *subtotal = subtotal
            .checked_add(panel.contained_kg)
            .map_err(arithmetic)?;
        let Some(obs) = &panel.observation else {
            continue;
        };
        if obs.available_at_ns > value.heap.as_of_ns {
            return Err(Refusal::new(
                RefusalCode::AssayLookahead108,
                format!("panel {id} solution evidence is unavailable at the cut"),
            ));
        }
        if lift.observation.is_some() {
            return Err(Refusal::new(
                RefusalCode::SupportMismatch102,
                format!(
                    "lift {} already has a cumulative leach observation",
                    panel.lift
                ),
            ));
        }
        if obs.start_ns < lift.irrigated_at_ns
            || obs.end_ns <= obs.start_ns
            || obs.end_ns > obs.available_at_ns
            || obs.pregnant_litres.signum() <= 0
            || obs.pregnant_mg_per_litre.signum() < 0
            || obs.barren_litres.signum() < 0
            || obs.barren_mg_per_litre.signum() < 0
            || obs.standard_deviation_kg.signum() <= 0
        {
            return Err(invalid(format!(
                "invalid solution observation for panel {id}"
            )));
        }
        let net = net_metal_kg(obs)?;
        if net.signum() < 0
            || net
                .checked_cmp(panel.contained_kg)
                .map_err(arithmetic)?
                .is_gt()
        {
            return Err(Refusal::new(
                RefusalCode::Conservation101,
                format!("panel {id} net collected metal is outside its declared population"),
            ));
        }
        measured.push(MeasuredPanel {
            id,
            panel,
            irrigated_at_ns: lift.irrigated_at_ns,
            observation: obs,
            windows,
        });
        evidence.push(GaussianObservation {
            value: net.to_f64(),
            sd: obs.standard_deviation_kg.to_f64(),
        });
    }
    for (id, lift) in &value.heap.lifts {
        if lift_totals.get(id) != Some(&lift.contained_kg) {
            return Err(Refusal::new(
                RefusalCode::Conservation101,
                format!("panel masses do not partition lift {id}"),
            ));
        }
    }
    if measured.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            "no panel has net solution evidence",
        ));
    }

    estimate_population(
        &value.heap,
        input,
        &evidence,
        |id, scenario, predictions| {
            for m in &measured {
                let curve = &scenario.curves[&m.panel.lift];
                let transport = &value.hydraulics[id][m.id];
                let rate = curve.rate_per_day.to_f64();
                let depth =
                    m.panel.drainage_depth_m.to_f64() * transport.mobile_water_fraction.to_f64();
                let channel = transport.channelled_fraction.to_f64();
                let obs = m.observation;
                let fast = collected_fraction(
                    age_days(obs.start_ns, m.irrigated_at_ns),
                    age_days(obs.end_ns, obs.start_ns),
                    rate,
                );
                let slow_start =
                    mobilization_age(&m.windows, m.irrigated_at_ns, obs.start_ns, depth);
                let slow_end = mobilization_age(&m.windows, m.irrigated_at_ns, obs.end_ns, depth);
                if slow_end < slow_start {
                    return Err(Refusal::new(
                        RefusalCode::NumericalCertificateFailed207,
                        format!("panel {} transport ages reversed", m.id),
                    ));
                }
                let slow = collected_fraction(slow_start, slow_end - slow_start, rate);
                let capacity = m.panel.contained_kg.to_f64() * curve.ultimate_fraction.to_f64();
                predictions.push(capacity * (channel * fast + (1.0 - channel) * slow));
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::heap_inventory::{
        LeachObservation,
        tests::{qty, request},
    };
    use fmine_claim::{Confidentiality, Representation, SpatialSupport};
    use fmine_core::{anchor::SourceAnchor, hash::sha256};
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

    const DAY: i64 = 86_400_000_000_000;

    fn fixture() -> PanelHeapInventoryInput {
        let panel = |lift: &str, contained| HeapPanel {
            lift: lift.into(),
            contained_kg: qty(contained),
            drainage_depth_m: qty("3"),
            application: vec![ApplicationPeriod {
                end_ns: 2 * DAY,
                litres_per_m2_hour: qty("125"),
            }],
            observation: None,
        };
        let mut north1 = panel("north", "600");
        north1.observation = Some(PanelSolutionObservation {
            start_ns: 0,
            end_ns: DAY,
            available_at_ns: 2 * DAY,
            pregnant_litres: qty("120000"),
            pregnant_mg_per_litre: qty("1000"),
            barren_litres: qty("15000"),
            barren_mg_per_litre: qty("1000"),
            standard_deviation_kg: qty("33.75"),
        });
        let panels = BTreeMap::from([
            ("north-1".into(), north1),
            ("north-2".into(), panel("north", "400")),
            ("south-1".into(), panel("south", "800")),
        ]);
        let hydraulic = |share| {
            panels
                .keys()
                .map(|id: &String| {
                    (
                        id.clone(),
                        PanelHydraulics {
                            mobile_water_fraction: qty("1"),
                            channelled_fraction: qty(share),
                        },
                    )
                })
                .collect()
        };
        let hydraulics = BTreeMap::from([
            ("north-favorable".into(), hydraulic("0.5")),
            ("south-favorable".into(), hydraulic("0.25")),
        ]);
        PanelHeapInventoryInput {
            heap: request(),
            panels,
            hydraulics,
        }
    }

    fn source(value: PanelHeapInventoryInput) -> Claim<PanelHeapInventoryInput> {
        let tag = value.canonical_tag();
        Claim::new(
            INPUT_METRIC,
            value,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![SourceAnchor::new(
                "synthetic-heap",
                sha256(tag.as_bytes()).0,
                "matched panel records",
            )],
        )
    }

    #[test]
    fn test_panel_solution_updates_odds_using_net_kg_and_drained_interval() {
        let mut fixture = fixture();
        let input = source(fixture.clone());
        let estimate = estimate_heap_inventory_from_panels(&input).unwrap();
        // Net measured = 120 − 15 = 105 kg. Flux = 3 m/day and slow-path lag = 1 day.
        // Day 1 collects only the fast share: 600 × 0.7 × 0.5 × 0.5 = 105 kg,
        // versus 600 × 0.5 × 0.5 × 0.25 = 37.5 kg, a two-SD difference.
        let weight = |estimate: &Claim<HeapInventoryEstimate>| {
            estimate.value.members["north-favorable"].weight.value()
        };
        assert!((weight(&estimate) - 1.0 / (1.0 + (-2.0_f64).exp())).abs() < 1e-12);
        assert_eq!(estimate.parents, [input.claim_id]);
        assert_eq!(estimate.confidentiality(), input.confidentiality());
        assert_eq!(estimate.state, KnowledgeState::Estimated);
        assert_eq!(estimate.value.observation_count, 1);
        assert_eq!(estimate.value.p50_kg.value(), 770.0);
        assert_eq!(
            estimate.value.members["north-favorable"]
                .remaining_by_lift_kg
                .as_ref()
                .unwrap()["north"],
            qty("490")
        );
        assert_eq!(
            estimate,
            estimate_heap_inventory_from_panels(&input).unwrap()
        );

        // On day 2 the fast increment is 1/4, and the slow increment is 1/2.
        // This gives 157.5 kg vs 131.25 kg, again two SDs apart at a different SD.
        let obs = fixture
            .panels
            .get_mut("north-1")
            .unwrap()
            .observation
            .as_mut()
            .unwrap();
        obs.start_ns = DAY;
        obs.end_ns = 2 * DAY;
        obs.pregnant_litres = qty("172500");
        obs.standard_deviation_kg = qty("13.125");
        let late = estimate_heap_inventory_from_panels(&source(fixture)).unwrap();
        assert!((weight(&late) - weight(&estimate)).abs() < 1e-12);
    }

    #[test]
    fn test_panel_solution_maps_slow_arrival_through_a_rate_change() {
        let mut fixture = fixture();
        let panel = fixture.panels.get_mut("north-1").unwrap();
        panel.application = vec![
            ApplicationPeriod {
                end_ns: DAY,
                litres_per_m2_hour: qty("125"),
            },
            ApplicationPeriod {
                end_ns: 2 * DAY,
                litres_per_m2_hour: qty("250"),
            },
        ];
        let obs = panel.observation.as_mut().unwrap();
        obs.start_ns = DAY;
        obs.end_ns = 2 * DAY;
        obs.pregnant_litres = qty("195000");
        obs.standard_deviation_kg = qty("20");
        let result = estimate_heap_inventory_from_panels(&source(fixture.clone())).unwrap();
        // The last 3 m of applied solution occupies half of day 2, not one
        // day. Slow arrivals through day 2 therefore cover source ages 0–1.5 days.
        let slow = 1.0 - 2.0_f64.powf(-1.5);
        let a = 420.0 * (0.5 * 0.25 + 0.5 * slow);
        let b = 300.0 * (0.25 * 0.25 + 0.75 * slow);
        let odds = (-0.5 * (((180.0 - b) / 20.0).powi(2) - ((180.0 - a) / 20.0).powi(2))).exp();
        let weight = result.value.members["north-favorable"].weight.value();
        assert!((weight - 1.0 / (1.0 + odds)).abs() < 1e-12);

        fixture.panels.get_mut("north-1").unwrap().application.pop();
        let err = estimate_heap_inventory_from_panels(&source(fixture)).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-109");
    }

    #[test]
    fn test_panel_solution_refuses_late_overlapping_and_nonconserving_inputs() {
        let mut fixture = fixture();
        let obs = fixture
            .panels
            .get_mut("north-1")
            .unwrap()
            .observation
            .as_mut()
            .unwrap();
        obs.available_at_ns += 1;
        let code = |request| {
            estimate_heap_inventory_from_panels(&source(request))
                .unwrap_err()
                .code
        };
        assert_eq!(code(fixture.clone()), "FMINE-REF-108");
        fixture
            .panels
            .get_mut("north-1")
            .unwrap()
            .observation
            .as_mut()
            .unwrap()
            .available_at_ns = 2 * DAY;
        fixture.heap.lifts.get_mut("north").unwrap().observation = Some(LeachObservation {
            observed_at_ns: DAY,
            available_at_ns: DAY,
            cumulative_fraction: qty("0.2"),
            standard_deviation: qty("0.025"),
        });
        assert_eq!(code(fixture.clone()), "FMINE-REF-102");
        fixture.heap.lifts.get_mut("north").unwrap().observation = None;
        fixture.panels.get_mut("north-2").unwrap().contained_kg = qty("401");
        assert_eq!(code(fixture.clone()), "FMINE-REF-101");
        fixture.panels.get_mut("north-2").unwrap().contained_kg = qty("400");
        fixture
            .panels
            .get_mut("north-1")
            .unwrap()
            .observation
            .as_mut()
            .unwrap()
            .barren_litres = qty("120001");
        assert_eq!(code(fixture), "FMINE-REF-101");
    }
}
