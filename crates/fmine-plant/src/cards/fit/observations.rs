//! DESIGN: derive historical correction baselines from the original processing
//! inputs and admitted card layers (plan §§17.3–17.4; BD-176/182). The shared
//! nominal arithmetic stops before any correction or cap. Its binary64 result
//! is represented exactly as a rational; no decimal rounding supplies a fit.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::reconciliation::represented_rational;

#[cfg(test)]
mod tests;

pub const OBSERVATION_METRIC: &str = "plant.recovery.correction_observation/1";
pub const BASELINE_METRIC: &str = "plant.recovery.pre_correction_baseline/1";
pub const BASELINE: &str = "evaluated_post_operational_pre_correction";
pub const NO_CLAIM: &str = "Historical baselines evaluate the original source-bound processing inputs with the declared test-work formula and ordered operational factors, before any correction or cap. The exact rational records the computed binary64 value, not exact empirical truth. Alignment and uncensored observation status remain source declarations. No representative population, uncertainty distribution, calibration, automatic model promotion or operating authorization is established.";

/// One source-declared plant observation and its complete simultaneous
/// processing inputs. The caller must separately establish physical alignment.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryObservation {
    pub id: String,
    pub inputs: Claim<RecoveryInputs>,
    pub available_ns: i64,
    pub observed_fraction: ExactQty,
    pub uncensored: bool,
}
impl RecoveryObservation {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCOB", 1);
        enc.put_str(&self.id);
        enc.put_str(&self.inputs.value.canonical_tag());
        inventory::put_source_envelope(&mut enc, &self.inputs, 0);
        enc.put_i64(self.available_ns);
        put_exact(&mut enc, self.observed_fraction);
        enc.put_u8(u8::from(self.uncensored));
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Intermediates preserve the exact order and binary64 arithmetic of nominal
/// evaluation. The last value is uncapped and may exceed one.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryBaseline {
    pub scope: RecoveryScope,
    pub interval: ValidTime,
    pub test_work_fraction: Fraction,
    pub operational_fractions: Vec<Fraction>,
    pub baseline_fraction: Fraction,
}

/// Original inputs, observation and all model layers remain inspectable. The
/// baseline Claim binds their complete envelopes into the derived pair's
/// ancestry, including metadata omitted from generic source Claim identities.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedRecoveryPairReceipt {
    model: RecoveryModel,
    observation: Claim<RecoveryObservation>,
    baseline: Claim<RecoveryBaseline>,
    claim: Claim<RecoveryPair>,
    digest: String,
}
impl DerivedRecoveryPairReceipt {
    pub fn model(&self) -> &RecoveryModel {
        &self.model
    }
    pub fn observation(&self) -> &Claim<RecoveryObservation> {
        &self.observation
    }
    pub fn baseline_claim(&self) -> &Claim<RecoveryBaseline> {
        &self.baseline
    }
    pub fn claim(&self) -> &Claim<RecoveryPair> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn test_work_fraction(&self) -> Fraction {
        self.baseline.value.test_work_fraction
    }
    pub fn operational_fractions(&self) -> &[Fraction] {
        &self.baseline.value.operational_fractions
    }
    pub fn baseline_fraction(&self) -> Fraction {
        self.baseline.value.baseline_fraction
    }
}

fn derivation_metadata(
    model: &RecoveryModel,
    observation: &Claim<RecoveryObservation>,
    enc: &mut Encoder,
) -> Result<Metadata> {
    let mut metadata = Metadata::new();
    include(&mut metadata, enc, model.test_work(), 0)?;
    enc.put_u64(model.operations().len() as u64);
    for operation in model.operations() {
        include(&mut metadata, enc, operation, 1)?;
    }
    // The original correction remains audit evidence, never baseline arithmetic.
    include(&mut metadata, enc, model.correction(), 2)?;
    include(&mut metadata, enc, model.route(), 3)?;
    include(&mut metadata, enc, &observation.value.inputs, 4)?;
    include(&mut metadata, enc, observation, 5)?;
    Ok(metadata)
}

/// Derive one uncapped post-operational pair without applying the original
/// correction, its cap or its fitted future interval. All ordinary input scope,
/// unit, exact validity and numerical checks still apply. Fitting separately
/// requires completed non-overlapping pairs available by its evidence cut.
pub fn derive_pair(
    model: &RecoveryModel,
    observation: &Claim<RecoveryObservation>,
) -> Result<DerivedRecoveryPairReceipt> {
    let source = &observation.value;
    if !named(&source.id) || source.id.len() > 256 || source.inputs.value.values.len() > MAX_INPUTS
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "historical recovery observations need a bounded identifier and coordinate count",
        ));
    }
    admit(observation, OBSERVATION_METRIC, &source.canonical_tag())?;
    if !source.uncensored {
        return Err(fail(
            RefusalCode::NotObservable202,
            "censored or capped observations cannot identify an uncapped recovery residual",
        ));
    }
    if source.observed_fraction.signum() < 0
        || compare(source.observed_fraction, ExactQty::from_integer(1))?.is_gt()
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "observed recovery must lie in [0,1]",
        ));
    }
    if source.available_ns < source.inputs.value.interval.end_ns {
        return Err(fail(
            RefusalCode::OutsideAoa203,
            "historical observation must be available after its processing interval completes",
        ));
    }
    let evaluated = model.evaluate_baseline(&source.inputs, false)?;
    let baseline_fraction = represented_rational(evaluated.result).map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "computed recovery baseline is outside the exact rational representation range",
        )
    })?;
    let pair = RecoveryPair {
        id: source.id.clone(),
        interval: source.inputs.value.interval,
        available_ns: source.available_ns,
        baseline_fraction,
        observed_fraction: source.observed_fraction,
        uncensored: source.uncensored,
        baseline_model: Some(BaselineModelBinding::from_model(model)),
    };
    let pair_tag = pair.canonical_tag();
    let mut enc = Encoder::new_frame(*b"RCBD", 1);
    let metadata = derivation_metadata(model, observation, &mut enc)?;
    enc.put_str(&pair_tag);
    enc.put_u64(evaluated.initial.to_bits());
    for value in &evaluated.operational_fractions {
        enc.put_u64(value.value().to_bits());
    }
    enc.put_u64(evaluated.result.to_bits());
    let digest = blake3_hash(&enc.finish()).to_hex();
    let baseline = metadata.into_claim(
        BASELINE_METRIC,
        RecoveryBaseline {
            scope: source.inputs.value.scope.clone(),
            interval: source.inputs.value.interval,
            test_work_fraction: Fraction::new(evaluated.initial),
            operational_fractions: evaluated.operational_fractions,
            baseline_fraction: Fraction::new(evaluated.result),
        },
        &digest,
    );
    let mut metadata =
        derivation_metadata(model, observation, &mut Encoder::new_frame(*b"RCBM", 1))?;
    metadata.include(&baseline)?;
    let claim = metadata.into_claim(PAIR_METRIC, pair, &pair_tag);
    Ok(DerivedRecoveryPairReceipt {
        model: model.clone(),
        observation: observation.clone(),
        baseline,
        claim,
        digest,
    })
}
