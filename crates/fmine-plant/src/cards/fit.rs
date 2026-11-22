//! DESIGN: exact descriptive additive recovery correction from source-bound
//! historical pairs (plan §17.3). Baselines are post-operational, pre-correction
//! and uncapped. Alignment is declared, never inferred from timestamps.
//! Determinism: Exact

use super::*;
use fmine_units::UnitsError;

#[cfg(test)]
mod tests;

pub const PAIR_METRIC: &str = "plant.recovery.correction_pair/1";
pub const INPUT_METRIC: &str = "plant.recovery.correction_fit_inputs/1";
pub const OUTPUT_METRIC: &str = "plant.recovery.correction_fit_summary/1";
pub const METHOD: &str = "unweighted_additive_residual_mean";
pub const BASELINE: &str = "post_operational_pre_correction";
pub const MAX_PAIRS: usize = 256;
pub const NO_CLAIM: &str = "Descriptive unweighted additive correction over explicitly aligned, source-bound historical pairs. Baselines are uncapped post-operational predictions before any old correction; the fitted correction replaces that correction. Sample residual variance describes the supplied residuals only. No independence, representative population, standard error, confidence coverage, probability calibration, automatic model promotion or operating authorization is established.";

/// The source declaration must establish which material the paired values
/// represent. A time match alone cannot establish equal processing support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitAlignment {
    SameProcessingSupport,
    ResidenceTimeAligned,
}
impl FitAlignment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SameProcessingSupport => "same_processing_support",
            Self::ResidenceTimeAligned => "residence_time_aligned",
        }
    }
    fn tag(self) -> u8 {
        match self {
            Self::SameProcessingSupport => 0,
            Self::ResidenceTimeAligned => 1,
        }
    }
}

/// One aligned, uncensored physical interval. Adjacent intervals may share an
/// endpoint; their interiors must not overlap. Every value uses recovery
/// fractions, not percentage points or tonnage weights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPair {
    pub id: String,
    pub interval: ValidTime,
    pub available_ns: i64,
    /// After all named operational factors, before correction or cap.
    pub baseline_fraction: ExactQty,
    pub observed_fraction: ExactQty,
    /// False rejects: a cap or censoring cannot identify an uncapped residual.
    pub uncensored: bool,
}
impl RecoveryPair {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCPF", 1);
        enc.put_str(&self.id);
        put_time(&mut enc, self.interval);
        enc.put_i64(self.available_ns);
        put_exact(&mut enc, self.baseline_fraction);
        put_exact(&mut enc, self.observed_fraction);
        enc.put_u8(u8::from(self.uncensored));
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// An explicit fit declaration bound to the original card and its full ordered
/// operational baseline. Pair order has no semantic meaning. Source Claims,
/// including their non-identity metadata, are retained in the canonical tag.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionFit {
    pub version: CardVersion,
    pub scope: RecoveryScope,
    pub test_work: ClaimId,
    pub operation_ids: Vec<ClaimId>,
    pub route: ClaimId,
    pub evidence_cut_ns: i64,
    pub target_interval: ValidTime,
    pub alignment: FitAlignment,
    pub upper_cap_fraction: Option<ExactQty>,
    pub pairs: Vec<Claim<RecoveryPair>>,
}
impl CorrectionFit {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCFI", 1);
        put_version(&mut enc, &self.version);
        put_scope(&mut enc, &self.scope);
        enc.put_str(&self.test_work.to_string());
        enc.put_u64(self.operation_ids.len() as u64);
        for id in &self.operation_ids {
            enc.put_str(&id.to_string());
        }
        enc.put_str(&self.route.to_string());
        enc.put_i64(self.evidence_cut_ns);
        put_time(&mut enc, self.target_interval);
        enc.put_u8(self.alignment.tag());
        enc.put_u8(u8::from(self.upper_cap_fraction.is_some()));
        if let Some(cap) = self.upper_cap_fraction {
            put_exact(&mut enc, cap);
        }
        let mut pairs: Vec<_> = self.pairs.iter().collect();
        pairs.sort_by(|a, b| a.value.id.cmp(&b.value.id));
        enc.put_u64(pairs.len() as u64);
        for pair in pairs {
            enc.put_str(&pair.value.canonical_tag());
            inventory::put_source_envelope(&mut enc, pair, 0);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryResidual {
    pub id: String,
    pub pair_id: ClaimId,
    pub residual_fraction: ExactQty,
}

/// All results use exact rational fractions. Variance is in squared recovery
/// fractions with denominator n-1, and is only a descriptive sample statistic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionFitSummary {
    pub pair_count: usize,
    pub mean_residual_fraction: ExactQty,
    pub sample_residual_variance: ExactQty,
    /// Chronological order, independent of input row order.
    pub residuals: Vec<RecoveryResidual>,
}
impl CorrectionFitSummary {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RCFS", 1);
        enc.put_u64(self.pair_count as u64);
        put_exact(&mut enc, self.mean_residual_fraction);
        put_exact(&mut enc, self.sample_residual_variance);
        enc.put_u64(self.residuals.len() as u64);
        for residual in &self.residuals {
            enc.put_str(&residual.id);
            enc.put_str(&residual.pair_id.to_string());
            put_exact(&mut enc, residual.residual_fraction);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// The original model, fit declaration and paired source Claims remain intact.
/// Applying the separately versioned correction is an explicit caller action.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionFitReceipt {
    model: RecoveryModel,
    input: Claim<CorrectionFit>,
    summary: Claim<CorrectionFitSummary>,
    correction: Claim<RecoveryCorrection>,
    digest: String,
}
impl CorrectionFitReceipt {
    pub fn model(&self) -> &RecoveryModel {
        &self.model
    }
    pub fn input(&self) -> &Claim<CorrectionFit> {
        &self.input
    }
    pub fn summary(&self) -> &Claim<CorrectionFitSummary> {
        &self.summary
    }
    pub fn correction(&self) -> &Claim<RecoveryCorrection> {
        &self.correction
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "correction-fit exact arithmetic overflows",
        )
    })
}

fn fit_metadata(
    model: &RecoveryModel,
    input: &Claim<CorrectionFit>,
    enc: &mut Encoder,
) -> Result<Metadata> {
    let mut metadata = Metadata::new();
    include(&mut metadata, enc, model.test_work(), 0)?;
    enc.put_u64(model.operations().len() as u64);
    for operation in model.operations() {
        include(&mut metadata, enc, operation, 1)?;
    }
    include(&mut metadata, enc, model.correction(), 2)?;
    include(&mut metadata, enc, model.route(), 3)?;
    include(&mut metadata, enc, input, 4)?;
    let mut pairs: Vec<_> = input.value.pairs.iter().collect();
    pairs.sort_by(|a, b| a.value.id.cmp(&b.value.id));
    enc.put_u64(pairs.len() as u64);
    for pair in pairs {
        include(&mut metadata, enc, pair, 5)?;
    }
    Ok(metadata)
}

/// Fit the exact unweighted mean of observed minus post-operational baseline
/// fractions. Original corrections are retained as evidence but never added to
/// the residuals. No card, forecast record or ledger is mutated by this function.
pub fn fit_correction(
    model: &RecoveryModel,
    input: &Claim<CorrectionFit>,
) -> Result<CorrectionFitReceipt> {
    let spec = &input.value;
    if !(2..=MAX_PAIRS).contains(&spec.pairs.len())
        || spec.operation_ids.len() > MAX_OPERATIONAL_FACTORS
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "correction fit requires 2..=256 pairs and bounded operational layers",
        ));
    }
    admit(input, INPUT_METRIC, &spec.canonical_tag())?;
    version(&spec.version)?;
    scope(&spec.scope)?;
    interval(spec.target_interval)?;
    if spec.version == model.correction().value.version {
        return Err(fail(
            RefusalCode::FrozenRecordMutation507,
            "fitted correction needs a new source version",
        ));
    }
    if spec.scope != model.test_work().value.scope
        || spec.test_work != model.test_work().claim_id
        || spec.route != model.route().claim_id
        || !spec
            .operation_ids
            .iter()
            .copied()
            .eq(model.operations().iter().map(|operation| operation.claim_id))
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "correction fit must name the complete model scope, test work, ordered operations and route",
        ));
    }
    let card_validity = model.test_work().value.valid_time;
    if spec.evidence_cut_ns >= spec.target_interval.start_ns
        || spec.target_interval.start_ns < card_validity.start_ns
        || spec.target_interval.end_ns > card_validity.end_ns
    {
        return Err(fail(
            RefusalCode::OutsideAoa203,
            "correction fit needs an evidence cut strictly before a target inside card validity",
        ));
    }
    if let Some(cap) = spec.upper_cap_fraction
        && (cap.signum() < 0 || compare(cap, ExactQty::from_integer(1))?.is_gt())
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "explicit fitted recovery cap must lie in [0,1]",
        ));
    }
    let mut pairs: Vec<_> = spec.pairs.iter().collect();
    pairs.sort_by(|a, b| {
        (a.value.interval.start_ns, &a.value.id)
            .cmp(&(b.value.interval.start_ns, &b.value.id))
    });
    let mut ids = BTreeSet::new();
    let mut previous_end = None;
    let mut residuals = Vec::with_capacity(pairs.len());
    let mut sum = ExactQty::from_integer(0);
    for pair in pairs {
        let value = &pair.value;
        admit(pair, PAIR_METRIC, &value.canonical_tag())?;
        interval(value.interval)?;
        if !named(&value.id) || value.id.len() > 256 || !ids.insert(value.id.clone()) {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "correction-fit pairs need distinct bounded source identifiers",
            ));
        }
        if previous_end.is_some_and(|end| value.interval.start_ns < end) {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "correction-fit pair intervals must not overlap",
            ));
        }
        previous_end = Some(value.interval.end_ns);
        if value.interval.start_ns < card_validity.start_ns
            || value.interval.end_ns > card_validity.end_ns
            || value.available_ns < value.interval.end_ns
            || value.available_ns > spec.evidence_cut_ns
            || value.interval.end_ns > spec.evidence_cut_ns
        {
            return Err(fail(
                RefusalCode::OutsideAoa203,
                "paired observations must be within card validity, available after completion and no later than the evidence cut",
            ));
        }
        if !value.uncensored {
            return Err(fail(
                RefusalCode::NotObservable202,
                "censored or capped pairs cannot identify the uncapped additive residual",
            ));
        }
        if value.baseline_fraction.signum() < 0
            || value.observed_fraction.signum() < 0
            || compare(value.observed_fraction, ExactQty::from_integer(1))?.is_gt()
        {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "paired recovery requires a nonnegative baseline and observation in [0,1]",
            ));
        }
        let residual = exact(value.observed_fraction.checked_sub(value.baseline_fraction))?;
        sum = exact(sum.checked_add(residual))?;
        residuals.push(RecoveryResidual {
            id: value.id.clone(),
            pair_id: pair.claim_id,
            residual_fraction: residual,
        });
    }
    let n = ExactQty::from_integer(residuals.len() as i128);
    let mean_residual_fraction = exact(sum.checked_div(n))?;
    let mut squared_deviations = ExactQty::from_integer(0);
    for residual in &residuals {
        let deviation = exact(residual.residual_fraction.checked_sub(mean_residual_fraction))?;
        let square = exact(deviation.checked_mul(deviation))?;
        squared_deviations = exact(squared_deviations.checked_add(square))?;
    }
    let denominator = ExactQty::from_integer((residuals.len() - 1) as i128);
    let sample_residual_variance = exact(squared_deviations.checked_div(denominator))?;
    let summary = CorrectionFitSummary {
        pair_count: residuals.len(),
        mean_residual_fraction,
        sample_residual_variance,
        residuals,
    };
    let mut enc = Encoder::new_frame(*b"RCFR", 1);
    let metadata = fit_metadata(model, input, &mut enc)?;
    enc.put_str(&summary.canonical_tag());
    let digest = blake3_hash(&enc.finish()).to_hex();
    let summary_tag = summary.canonical_tag();
    let summary = metadata.into_claim(OUTPUT_METRIC, summary, &summary_tag);
    let value = RecoveryCorrection {
        version: spec.version.clone(),
        test_work: spec.test_work,
        additive_fraction: mean_residual_fraction,
        upper_cap_fraction: spec.upper_cap_fraction,
        fit_validity: Some(CorrectionFitValidity {
            evidence_cut_ns: spec.evidence_cut_ns,
            target_interval: spec.target_interval,
            operation_ids: spec.operation_ids.clone(),
            route: spec.route,
            fit_digest: digest.clone(),
        }),
    };
    let tag = value.canonical_tag();
    let mut metadata = fit_metadata(model, input, &mut Encoder::new_frame(*b"RCFM", 1))?;
    metadata.include(&summary)?;
    let correction = metadata.into_claim(CORRECTION_METRIC, value, &tag);
    model.with_correction(&correction)?;
    Ok(CorrectionFitReceipt {
        model: model.clone(),
        input: input.clone(),
        summary,
        correction,
        digest,
    })
}
