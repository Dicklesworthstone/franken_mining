//! DESIGN: propagate a declared joint processing-blend population through one
//! compiled recovery model (plan §17.4; BD-176/182). Member identities preserve
//! dependence between coordinates; no independent draws or fitted probabilities.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Representation, SpatialSupport};
use fmine_evidence::KnowledgeState;

mod render;
#[cfg(test)]
mod tests;

pub const INPUT_METRIC: &str = "plant.recovery.joint_population/1";
pub const OUTPUT_METRIC: &str = "plant.recovery.ensemble_evaluation/1";
pub const MAX_SCENARIOS: usize = 256;
pub const NO_CLAIM: &str = "Conditional finite-population recovery distribution under the declared joint inputs, weights and common versioned card. No probability calibration, input-population completeness in reality, fitted model uncertainty, sample-to-blend transfer or operating authorization is established.";

/// One complete simultaneous blend realization and its exact probability.
/// Weights are probabilities across alternative states, never tonnage weights
/// for blending different physical lots or intervals.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryScenario {
    pub id: String,
    pub weight: ExactQty,
    pub inputs: Claim<RecoveryInputs>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryPopulation {
    pub scenarios: Vec<RecoveryScenario>,
}
impl RecoveryPopulation {
    /// Bind every named joint row, including its original input envelope.
    /// Declaration order has no semantic meaning; duplicate IDs still refuse.
    pub fn canonical_tag(&self) -> String {
        let mut scenarios: Vec<_> = self.scenarios.iter().collect();
        scenarios.sort_by(|a, b| a.id.cmp(&b.id));
        let mut enc = Encoder::new_frame(*b"RCEP", 1);
        enc.put_u64(scenarios.len() as u64);
        for scenario in scenarios {
            enc.put_str(&scenario.id);
            put_exact(&mut enc, scenario.weight);
            enc.put_str(&scenario.inputs.value.canonical_tag());
            inventory::put_source_envelope(&mut enc, &scenario.inputs, 0);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquaredRecoveryFraction;
pub type FractionVariance = Qty<SquaredRecoveryFraction>;

/// Population moments, not sample estimators or confidence bounds. Quantiles
/// are inf {r: exact cumulative probability at r >= p}, without interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEnsembleSummary {
    pub scope: RecoveryScope,
    pub interval: ValidTime,
    pub route: String,
    pub scenario_count: usize,
    pub mean: Fraction,
    pub variance: FractionVariance,
    pub minimum: Fraction,
    pub p10: Fraction,
    pub p50: Fraction,
    pub p90: Fraction,
    pub maximum: Fraction,
    pub probability_capped: ExactQty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryMemberReceipt {
    id: String,
    weight: ExactQty,
    receipt: RecoveryReceipt,
}
impl RecoveryMemberReceipt {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn weight(&self) -> ExactQty {
        self.weight
    }
    pub fn receipt(&self) -> &RecoveryReceipt {
        &self.receipt
    }
}

/// Immutable complete source population and individually admitted evaluations.
/// Every member is retained, including explicit cap application and all layers.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEnsembleReceipt {
    model: RecoveryModel,
    population: Claim<RecoveryPopulation>,
    members: Vec<RecoveryMemberReceipt>,
    claim: Claim<RecoveryEnsembleSummary>,
    digest: String,
}
impl RecoveryEnsembleReceipt {
    pub fn model(&self) -> &RecoveryModel {
        &self.model
    }
    pub fn population(&self) -> &Claim<RecoveryPopulation> {
        &self.population
    }
    pub fn members(&self) -> &[RecoveryMemberReceipt] {
        &self.members
    }
    pub fn claim(&self) -> &Claim<RecoveryEnsembleSummary> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl RecoveryModel {
    /// Evaluate every complete joint realization with the same compiled card.
    /// Any invalid member refuses the whole population, without dropping,
    /// extrapolating, renormalizing or replacing it with a nominal input.
    pub fn evaluate_ensemble(
        &self,
        population: &Claim<RecoveryPopulation>,
    ) -> Result<RecoveryEnsembleReceipt> {
        let scenarios = &population.value.scenarios;
        if scenarios.is_empty() || scenarios.len() > MAX_SCENARIOS {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "joint population requires 1 through 256 complete scenarios",
            ));
        }
        admit(population, INPUT_METRIC, &population.value.canonical_tag())?;
        let mut scenarios: Vec<_> = scenarios.iter().collect();
        scenarios.sort_by(|a, b| a.id.cmp(&b.id));
        let mut total = ExactQty::from_integer(0);
        let interval = scenarios[0].inputs.value.interval;
        for (i, scenario) in scenarios.iter().enumerate() {
            if !named(&scenario.id) || scenario.id.len() > 256 {
                return Err(fail(
                    RefusalCode::NoClaimInDomain201,
                    "scenario IDs must be nonblank, control-free names of at most 256 bytes",
                ));
            }
            if i > 0 && scenarios[i - 1].id == scenario.id {
                return Err(fail(
                    RefusalCode::BasisMismatch105,
                    format!("duplicate joint scenario {}", scenario.id),
                ));
            }
            if scenario.weight.signum() <= 0 {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "scenario {} requires strictly positive probability",
                        scenario.id
                    ),
                ));
            }
            total = exact_sum(total, scenario.weight)?;
            if scenario.inputs.value.interval != interval {
                return Err(fail(
                    RefusalCode::SupportMismatch102,
                    "joint realizations must describe the same processing interval",
                ));
            }
        }
        if total != ExactQty::from_integer(1) {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "joint probabilities must sum exactly to one; submitted {}/{}",
                    total.num(),
                    total.den()
                ),
            ));
        }

        let mut members = Vec::with_capacity(scenarios.len());
        let mut probability_capped = ExactQty::from_integer(0);
        let mut metadata = Metadata::new();
        let mut enc = Encoder::new_frame(*b"RCES", 1);
        include(&mut metadata, &mut enc, population, 0)?;
        enc.put_u64(scenarios.len() as u64);
        for scenario in scenarios {
            let receipt = self.evaluate(&scenario.inputs).map_err(|mut refusal| {
                refusal.what = format!("scenario {}: {}", scenario.id, refusal.what);
                refusal
            })?;
            if receipt.claim().value.cap_applied {
                probability_capped = exact_sum(probability_capped, scenario.weight)?;
            }
            enc.put_str(&scenario.id);
            put_exact(&mut enc, scenario.weight);
            enc.put_str(receipt.digest());
            include(&mut metadata, &mut enc, receipt.claim(), 1)?;
            members.push(RecoveryMemberReceipt {
                id: scenario.id.clone(),
                weight: scenario.weight,
                receipt,
            });
        }
        let mut ordered: Vec<_> = members.iter().collect();
        ordered.sort_by(|a, b| {
            recovery(a)
                .total_cmp(&recovery(b))
                .then_with(|| a.id.cmp(&b.id))
        });
        let minimum = recovery(ordered[0]);
        let maximum = recovery(ordered[ordered.len() - 1]);
        // A nonnegative shifted sum preserves constant populations exactly and
        // avoids cancellation when a very small probability owns the only loss.
        let mut mean_shift = Sum::default();
        for member in &members {
            mean_shift.add(product(
                numeric(member.weight)?,
                recovery(member) - minimum,
            )?);
        }
        let mean = finite(minimum + mean_shift.finish()?)?;
        if mean < minimum || mean > maximum {
            return Err(fail(
                RefusalCode::NumericalCertificateFailed207,
                "weighted mean loses its numerical population range",
            ));
        }
        let mut variance = Sum::default();
        for member in &members {
            let difference = recovery(member) - mean;
            variance.add(product(
                numeric(member.weight)?,
                product(difference, difference)?,
            )?);
        }
        let variance = variance.finish()?;
        let p10 = quantile(&ordered, ExactQty::new(1, 10).map_err(arithmetic)?)?;
        let p50 = quantile(&ordered, ExactQty::new(1, 2).map_err(arithmetic)?)?;
        let p90 = quantile(&ordered, ExactQty::new(9, 10).map_err(arithmetic)?)?;
        let value = RecoveryEnsembleSummary {
            scope: self.test_work.value.scope.clone(),
            interval,
            route: self.route.value.route.clone(),
            scenario_count: members.len(),
            mean: Fraction::new(mean),
            variance: FractionVariance::new(variance),
            minimum: Fraction::new(minimum),
            p10,
            p50,
            p90,
            maximum: Fraction::new(maximum),
            probability_capped,
        };
        for number in [
            mean,
            variance,
            minimum,
            p10.value(),
            p50.value(),
            p90.value(),
            maximum,
        ] {
            enc.put_u64(number.to_bits());
        }
        put_exact(&mut enc, probability_capped);
        let digest = blake3_hash(&enc.finish()).to_hex();
        let claim = Claim::new(
            OUTPUT_METRIC,
            value,
            &digest,
            Representation::EnsembleSummary {
                n: members.len() as u32,
                mean,
                p10: p10.value(),
                p50: p50.value(),
                p90: p90.value(),
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
        );
        Ok(RecoveryEnsembleReceipt {
            model: self.clone(),
            population: population.clone(),
            members,
            claim,
            digest,
        })
    }
}

fn recovery(member: &RecoveryMemberReceipt) -> f64 {
    member.receipt.claim().value.recovery_fraction.value()
}
fn arithmetic(_: fmine_units::UnitsError) -> Refusal {
    fail(
        RefusalCode::NumericalCertificateFailed207,
        "exact joint probability arithmetic overflows",
    )
}
fn exact_sum(a: ExactQty, b: ExactQty) -> Result<ExactQty> {
    a.checked_add(b).map_err(arithmetic)
}
fn product(a: f64, b: f64) -> Result<f64> {
    let result = finite(a * b)?;
    if a != 0.0 && b != 0.0 && result == 0.0 {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "joint recovery moment underflows",
        ));
    }
    Ok(result)
}
fn quantile(members: &[&RecoveryMemberReceipt], probability: ExactQty) -> Result<Fraction> {
    let mut cumulative = ExactQty::from_integer(0);
    for member in members {
        cumulative = exact_sum(cumulative, member.weight)?;
        if !compare(cumulative, probability)?.is_lt() {
            return Ok(member.receipt.claim().value.recovery_fraction);
        }
    }
    Err(fail(
        RefusalCode::NumericalCertificateFailed207,
        "joint quantile has no probability support",
    ))
}

/// Fixed-order Neumaier accumulation; no subtracting two nearly equal moments.
#[derive(Default)]
struct Sum {
    sum: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }
    fn finish(self) -> Result<f64> {
        finite(self.sum + self.correction)
    }
}
