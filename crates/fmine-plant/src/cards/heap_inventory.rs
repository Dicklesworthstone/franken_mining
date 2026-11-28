//! DESIGN (BD-183, GB-20): finite joint leach scenarios for a pad at a declared cut.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, det, hash::blake3_hash};
use fmine_evidence::{KnowledgeState, Refusal, RefusalCode};
use fmine_units::{ExactQty, Qty, UnitsError};

use super::{Result, put_exact};
use crate::{
    inventory,
    reconciliation::{Kilograms, Metadata},
};

pub const INPUT_METRIC: &str = "plant.heap.inventory_inputs/1";
pub const OUTPUT_METRIC: &str = "plant.heap.recoverable_inventory/1";
const MAX_LIFTS: usize = 64;
const MAX_SCENARIOS: usize = 256;
const NS_PER_DAY: f64 = 86_400_000_000_000.0;

/// Cumulative fraction mobilized since irrigation started. The declared
/// error is Gaussian on this fraction. Observations across lifts are assumed
/// conditionally independent within a scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeachObservation {
    pub observed_at_ns: i64,
    pub available_at_ns: i64,
    pub cumulative_fraction: ExactQty,
    pub standard_deviation: ExactQty,
}

/// Exact nominal metal kg on a single lift. Recovered and lost are disjoint,
/// cumulative debits through the pad's as-of cut. Neither is inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapLift {
    pub irrigated_at_ns: i64,
    pub contained_kg: ExactQty,
    pub recovered_kg: ExactQty,
    pub lost_kg: ExactQty,
    pub observation: Option<LeachObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeachCurve {
    pub ultimate_fraction: ExactQty,
    pub rate_per_day: ExactQty,
}

/// One simultaneous state of every lift. Relative prior weights are positive
/// integers. A member's curves travel together through the update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapScenario {
    pub weight: u32,
    pub curves: BTreeMap<String, LeachCurve>,
}

/// One element and a declared complete lift population, through an as-of cut
/// in UTC nanoseconds. This is a closed-pad estimate over the submitted lifts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapInventoryInput {
    pub pad: String,
    pub element: String,
    pub as_of_ns: i64,
    pub lifts: BTreeMap<String, HeapLift>,
    pub scenarios: BTreeMap<String, HeapScenario>,
}

impl HeapInventoryInput {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"HPIN", 1);
        enc.put_str(&self.pad);
        enc.put_str(&self.element);
        enc.put_i64(self.as_of_ns);
        enc.put_u64(self.lifts.len() as u64);
        for (id, lift) in &self.lifts {
            enc.put_str(id);
            enc.put_i64(lift.irrigated_at_ns);
            put_exact(&mut enc, lift.contained_kg);
            put_exact(&mut enc, lift.recovered_kg);
            put_exact(&mut enc, lift.lost_kg);
            enc.put_u8(u8::from(lift.observation.is_some()));
            if let Some(observation) = &lift.observation {
                enc.put_i64(observation.observed_at_ns);
                enc.put_i64(observation.available_at_ns);
                put_exact(&mut enc, observation.cumulative_fraction);
                put_exact(&mut enc, observation.standard_deviation);
            }
        }
        enc.put_u64(self.scenarios.len() as u64);
        for (id, scenario) in &self.scenarios {
            enc.put_str(id);
            enc.put_u32(scenario.weight);
            enc.put_u64(scenario.curves.len() as u64);
            for (lift, curve) in &scenario.curves {
                enc.put_str(lift);
                put_exact(&mut enc, curve.ultimate_fraction);
                put_exact(&mut enc, curve.rate_per_day);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionalProbability;
pub type Probability = Qty<ConditionalProbability>;

#[derive(Debug, Clone, PartialEq)]
pub struct HeapPosteriorMember {
    pub weight: Probability,
    /// Absent if the scenario cannot cover the recorded metal debits.
    pub remaining_by_lift_kg: Option<BTreeMap<String, ExactQty>>,
}

/// Recoverable metal, conditional on the declared first-order model, source
/// population and error assumptions. Summaries are computed over joint pad
/// totals. No calibrated coverage, book valuation or execution authority.
#[derive(Debug, Clone, PartialEq)]
pub struct HeapInventoryEstimate {
    pub pad: String,
    pub element: String,
    pub as_of_ns: i64,
    pub observation_count: usize,
    pub members: BTreeMap<String, HeapPosteriorMember>,
    pub mean_kg: Kilograms,
    pub p10_kg: Kilograms,
    pub p50_kg: Kilograms,
    pub p90_kg: Kilograms,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn arithmetic(error: UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        error.to_string(),
    )
}

fn is_fraction(value: ExactQty) -> bool {
    (0..=value.den()).contains(&value.num())
}

fn named(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.trim() == id && !id.chars().any(char::is_control)
}

fn debits(id: &str, lift: &HeapLift, as_of_ns: i64) -> Result<ExactQty> {
    if !named(id)
        || lift.contained_kg.signum() <= 0
        || lift.recovered_kg.signum() < 0
        || lift.lost_kg.signum() < 0
        || lift.irrigated_at_ns > as_of_ns
    {
        return Err(invalid(format!("invalid heap lift {id:?}")));
    }
    let debit = lift
        .recovered_kg
        .checked_add(lift.lost_kg)
        .map_err(arithmetic)?;
    if lift
        .contained_kg
        .checked_cmp(debit)
        .map_err(arithmetic)?
        .is_lt()
        || (lift.irrigated_at_ns == as_of_ns && debit.signum() > 0)
    {
        return Err(Refusal::new(
            RefusalCode::Conservation101,
            format!("lift {id} has metal debits outside its available population"),
        ));
    }
    if let Some(obs) = &lift.observation {
        if obs.available_at_ns > as_of_ns {
            return Err(Refusal::new(
                RefusalCode::AssayLookahead108,
                format!("lift {id} leach observation is unavailable at the as-of cut"),
            ));
        }
        if obs.observed_at_ns <= lift.irrigated_at_ns
            || obs.observed_at_ns > obs.available_at_ns
            || !is_fraction(obs.cumulative_fraction)
            || obs.standard_deviation.signum() <= 0
        {
            return Err(invalid(format!("invalid leach observation for lift {id}")));
        }
    }
    Ok(debit)
}

/// Update a joint scenario population with leach observations, then deduct the
/// recorded metal debits from ultimate recoverable kg. Impossible scenarios
/// retain their identity with zero posterior mass. The input Claim binds all
/// observations, dispositions and model parameters.
pub fn estimate_heap_inventory(
    input: &Claim<HeapInventoryInput>,
) -> Result<Claim<HeapInventoryEstimate>> {
    let value = &input.value;
    if !named(&value.pad)
        || !named(&value.element)
        || !(1..=MAX_LIFTS).contains(&value.lifts.len())
        || !(1..=MAX_SCENARIOS).contains(&value.scenarios.len())
        || value
            .scenarios
            .values()
            .any(|scenario| scenario.curves.len() != value.lifts.len())
    {
        return Err(invalid(
            "heap inventory requires a named pad and element and a bounded complete joint lift population",
        ));
    }
    inventory::admit_source(
        input,
        INPUT_METRIC,
        &value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let mut metadata = Metadata::new();
    metadata.include(input)?;
    let debits = value
        .lifts
        .iter()
        .map(|(id, lift)| debits(id, lift, value.as_of_ns).map(|debit| (id, debit)))
        .collect::<Result<BTreeMap<_, _>>>()?;

    let mut evaluated = Vec::with_capacity(value.scenarios.len());
    for (id, scenario) in &value.scenarios {
        if !named(id) || scenario.weight == 0 || !scenario.curves.keys().eq(value.lifts.keys()) {
            return Err(invalid(format!(
                "scenario {id:?} requires positive weight and exactly one curve per lift"
            )));
        }
        let mut score = det::ln(f64::from(scenario.weight));
        let mut remaining = BTreeMap::new();
        for ((lift_id, lift), curve) in value.lifts.iter().zip(scenario.curves.values()) {
            if !is_fraction(curve.ultimate_fraction) || curve.rate_per_day.signum() <= 0 {
                return Err(invalid(format!(
                    "invalid scenario {id} curve for lift {lift_id}"
                )));
            }
            let balance = lift
                .contained_kg
                .checked_mul(curve.ultimate_fraction)
                .and_then(|ultimate| ultimate.checked_sub(debits[lift_id]))
                .map_err(arithmetic)?;
            remaining.insert(lift_id.clone(), balance);
            if let Some(obs) = &lift.observation {
                let days = (i128::from(obs.observed_at_ns) - i128::from(lift.irrigated_at_ns))
                    as f64
                    / NS_PER_DAY;
                let fraction = -curve.ultimate_fraction.to_f64()
                    * det::expm1(-curve.rate_per_day.to_f64() * days);
                let residual =
                    (obs.cumulative_fraction.to_f64() - fraction) / obs.standard_deviation.to_f64();
                score -= 0.5 * residual * residual;
            }
        }
        let feasible = if remaining.values().all(|mass| mass.signum() >= 0) {
            let total = remaining
                .values()
                .try_fold(ExactQty::from_integer(0), |sum, &kg| sum.checked_add(kg))
                .map_err(arithmetic)?;
            Some((remaining, total))
        } else {
            None
        };
        evaluated.push((id, feasible, score));
    }
    let max_score = evaluated
        .iter()
        .filter(|(_, balance, _)| balance.is_some())
        .map(|(_, _, score)| *score)
        .max_by(f64::total_cmp)
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NotObservable202,
                "no heap scenario can cover the recorded metal debits",
            )
        })?;
    let scaled = |score: f64| det::exp(score - max_score);
    let normalizer: f64 = evaluated
        .iter()
        .filter(|(_, balance, _)| balance.is_some())
        .map(|(_, _, score)| scaled(*score))
        .sum();
    let mut totals = Vec::new();
    let mut members = BTreeMap::new();
    let mut enc = Encoder::new_frame(*b"HPRC", 1);
    inventory::put_source_envelope(&mut enc, input, 1);
    for (id, balance, score) in evaluated {
        enc.put_str(id);
        enc.put_u8(u8::from(balance.is_some()));
        let (balances, probability) = if let Some((balances, total)) = balance {
            let probability = scaled(score) / normalizer;
            totals.push((total.to_f64(), probability));
            put_exact(&mut enc, total);
            (Some(balances), probability)
        } else {
            (None, 0.0)
        };
        enc.put_u64(probability.to_bits());
        members.insert(
            id.clone(),
            HeapPosteriorMember {
                weight: Probability::new(probability),
                remaining_by_lift_kg: balances,
            },
        );
    }
    totals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mean: f64 = totals.iter().map(|(total, weight)| total * weight).sum();
    let quantile = |p| {
        let mut sum = 0.0;
        totals
            .iter()
            .find(|(_, weight)| {
                sum += *weight;
                sum >= p
            })
            .map(|(total, _)| *total)
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::NumericalCertificateFailed207,
                    "heap posterior has no normalized quantile",
                )
            })
    };
    let p10 = quantile(0.1)?;
    let p50 = quantile(0.5)?;
    let p90 = quantile(0.9)?;
    let summary = HeapInventoryEstimate {
        pad: value.pad.clone(),
        element: value.element.clone(),
        as_of_ns: value.as_of_ns,
        observation_count: value
            .lifts
            .values()
            .filter(|lift| lift.observation.is_some())
            .count(),
        members,
        mean_kg: Kilograms::new(mean),
        p10_kg: Kilograms::new(p10),
        p50_kg: Kilograms::new(p50),
        p90_kg: Kilograms::new(p90),
    };
    Ok(Claim::new(
        OUTPUT_METRIC,
        summary,
        &blake3_hash(&enc.finish()).to_hex(),
        Representation::EnsembleSummary {
            n: totals.len() as u32,
            mean,
            p10,
            p50,
            p90,
        },
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        metadata.evidence,
        metadata.epistemic,
        metadata.state.and(KnowledgeState::Estimated),
        metadata.confidentiality,
        metadata.uu.into_values().collect(),
        metadata.parents.into_iter().collect(),
        metadata.anchors.into_iter().collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::Confidentiality;
    use fmine_core::{anchor::SourceAnchor, hash::sha256};
    use fmine_evidence::{EpistemicStatus, EvidenceClass};

    const DAY: i64 = 86_400_000_000_000;

    fn qty(text: &str) -> ExactQty {
        ExactQty::from_decimal_str(text).unwrap()
    }

    fn lift(contained: i128, recovered: i128, lost: i128) -> HeapLift {
        HeapLift {
            irrigated_at_ns: 0,
            contained_kg: ExactQty::from_integer(contained),
            recovered_kg: ExactQty::from_integer(recovered),
            lost_kg: ExactQty::from_integer(lost),
            observation: None,
        }
    }

    fn scenario(north: &str, south: &str) -> HeapScenario {
        let curve = |fraction| LeachCurve {
            ultimate_fraction: qty(fraction),
            rate_per_day: qty("0.6931471805599453"),
        };
        HeapScenario {
            weight: 1,
            curves: BTreeMap::from([
                ("north".into(), curve(north)),
                ("south".into(), curve(south)),
            ]),
        }
    }

    fn request() -> HeapInventoryInput {
        HeapInventoryInput {
            pad: "synthetic-pad".into(),
            element: "Au".into(),
            as_of_ns: 2 * DAY,
            lifts: BTreeMap::from([
                ("north".into(), lift(1000, 200, 10)),
                ("south".into(), lift(800, 100, 20)),
            ]),
            scenarios: BTreeMap::from([
                ("north-favorable".into(), scenario("0.7", "0.5")),
                ("south-favorable".into(), scenario("0.5", "0.6")),
            ]),
        }
    }

    fn source(value: HeapInventoryInput) -> Claim<HeapInventoryInput> {
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
                "submitted lifts",
            )],
        )
    }

    #[test]
    fn test_heap_joint_leach_update_recovers_independent_known_answer() {
        let mut request = request();
        let prior = estimate_heap_inventory(&source(request.clone())).unwrap();
        // Independent hand balances: 490 + 280 = 770 kg and 290 + 360 = 650 kg.
        assert_eq!(prior.value.mean_kg.value(), 710.0);
        assert_eq!(prior.value.p10_kg.value(), 650.0);
        assert_eq!(prior.value.p50_kg.value(), 650.0);
        assert_eq!(prior.value.p90_kg.value(), 770.0);
        assert_eq!(prior.value.observation_count, 0);
        request.lifts.get_mut("north").unwrap().observation = Some(LeachObservation {
            observed_at_ns: DAY,
            available_at_ns: 2 * DAY,
            cumulative_fraction: qty("0.35"),
            standard_deviation: qty("0.025"),
        });
        let input = source(request);
        let posterior = estimate_heap_inventory(&input).unwrap();
        let favorable = &posterior.value.members["north-favorable"];
        // At one day, the ln(2)/day curves predict 0.35 and 0.25.
        // The second model is four standard deviations away, so odds are 1 : exp(-8).
        let expected_weight = 1.0 / (1.0 + (-8.0_f64).exp());
        assert!((favorable.weight.value() - expected_weight).abs() < 1e-12);
        assert!(
            (posterior.value.mean_kg.value() - (650.0 + 120.0 * expected_weight)).abs() < 1e-10
        );
        assert_eq!(posterior.value.p10_kg.value(), 770.0);
        assert_eq!(posterior.value.p90_kg.value(), 770.0);
        assert_eq!(posterior.value.observation_count, 1);
        assert_eq!(
            favorable.remaining_by_lift_kg.as_ref().unwrap(),
            &BTreeMap::from([("north".into(), qty("490")), ("south".into(), qty("280"))])
        );
        assert_eq!(posterior.parents, [input.claim_id]);
        assert_eq!(posterior.anchors, input.anchors);
        assert_eq!(posterior.confidentiality(), input.confidentiality());
        assert_eq!(posterior.state, KnowledgeState::Estimated);
        assert_eq!(posterior.evidence, EvidenceClass::Assumed);
        assert_eq!(posterior, estimate_heap_inventory(&input).unwrap());
    }

    #[test]
    fn test_heap_impossible_scenarios_are_explicit_without_negative_inventory() {
        let mut request = request();
        request.lifts.get_mut("north").unwrap().recovered_kg = qty("600");
        let posterior = estimate_heap_inventory(&source(request.clone())).unwrap();
        let impossible = &posterior.value.members["south-favorable"];
        assert_eq!(impossible.weight.value(), 0.0);
        assert!(impossible.remaining_by_lift_kg.is_none());
        assert_eq!(posterior.value.p50_kg.value(), 370.0);
        request.lifts.get_mut("north").unwrap().recovered_kg = qty("800");
        assert_eq!(
            estimate_heap_inventory(&source(request.clone()))
                .unwrap_err()
                .code,
            "FMINE-REF-202"
        );
        request.lifts.get_mut("north").unwrap().recovered_kg = qty("991");
        assert_eq!(
            estimate_heap_inventory(&source(request)).unwrap_err().code,
            "FMINE-REF-101"
        );
    }

    #[test]
    fn test_heap_refuses_lookahead_wrong_lift_and_mutated_sources() {
        let mut request = request();
        request.lifts.get_mut("north").unwrap().observation = Some(LeachObservation {
            observed_at_ns: DAY,
            available_at_ns: 2 * DAY + 1,
            cumulative_fraction: qty("0.35"),
            standard_deviation: qty("0.025"),
        });
        assert_eq!(
            estimate_heap_inventory(&source(request.clone()))
                .unwrap_err()
                .code,
            "FMINE-REF-108"
        );
        request.lifts.get_mut("north").unwrap().observation = None;
        let mut changed = source(request.clone());
        changed.value.as_of_ns += 1;
        assert_eq!(
            estimate_heap_inventory(&changed).unwrap_err().code,
            "FMINE-REF-507"
        );
        let curves = &mut request.scenarios.get_mut("north-favorable").unwrap().curves;
        let wrong_lift = curves.remove("south").unwrap();
        curves.insert("elsewhere".into(), wrong_lift);
        assert_eq!(
            estimate_heap_inventory(&source(request)).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
}
