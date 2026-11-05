//! DESIGN: compare tail metal with RTD-aligned passive feed-equivalent metal.
//! This is conditional delay-then-separation recovery, not an inverse solution
//! for the RTD, a chemical leach model, measurement-error fit or calibrated card.
//! The original feed and tail observations are retained, including the misleading
//! same-interval ratio. No unrecorded loss is silently called recovered metal.
//! Determinism: DeterministicPerIsa

use super::*;

pub const DECLARATION_METRIC: &str = "plant.rtd.recovery_declaration/1";
pub const TAIL_METRIC: &str = "plant.rtd.tail_element/1";
pub const OUTPUT_METRIC: &str = "plant.rtd.conditional_recovery/1";

/// Explicit physical hypothesis, distinct from the passive transport model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryDeclaration {
    pub transport_claim: ClaimId,
    pub element: ElementKey,
    /// Recovery acts after the declared common passive transport boundary.
    /// Distributed reaction, preferential adsorption and evolving kinetics
    /// require their own transport/reaction models instead of this comparison.
    pub delay_then_separation: bool,
    pub complete_tail_population: bool,
    /// No unobserved product, spill or chemical loss is counted as recovery.
    pub all_non_tail_element_is_recovered: bool,
}
impl RecoveryDeclaration {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTRD", 1);
        enc.put_str(&self.transport_claim.to_string());
        put_element(&mut enc, &self.element);
        enc.put_u8(u8::from(self.delay_then_separation));
        enc.put_u8(u8::from(self.complete_tail_population));
        enc.put_u8(u8::from(self.all_non_tail_element_is_recovered));
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Full physical tailing elemental mass integrated over exactly one interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailObservation {
    pub scope: FlowsheetScope,
    pub element: ElementKey,
    pub tail_element_kg: ExactQty,
}
impl TailObservation {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTDT", 1);
        put_scope(&mut enc, &self.scope);
        put_element(&mut enc, &self.element);
        put_qty(&mut enc, self.tail_element_kg);
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntervalRecovery {
    pub interval: ValidTime,
    pub tail_source: ClaimId,
    pub tail_element_kg: Kilograms,
    pub same_interval_feed_element_kg: Kilograms,
    pub aligned_feed_equivalent_kg: Kilograms,
    /// Diagnostic only. It can be negative when retained high-grade feed exits.
    /// None means zero same-interval feed; no ratio is fabricated.
    pub same_interval_recovery: Option<f64>,
    /// Conditional recovered fraction under the explicit separation declaration.
    /// None means both aligned metal and observed tail metal are zero.
    pub lag_aligned_recovery: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryComparison {
    pub scope: FlowsheetScope,
    pub element: ElementKey,
    pub intervals: Vec<IntervalRecovery>,
}

/// Borrows the immutable transport receipt instead of copying its full history
/// for each elemental comparison. Original tail Claims remain separately owned.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryReceipt<'a> {
    transport: &'a RtdReceipt,
    declaration: Claim<RecoveryDeclaration>,
    observations: Vec<Claim<TailObservation>>,
    claim: Claim<RecoveryComparison>,
    digest: String,
}
impl<'a> RecoveryReceipt<'a> {
    pub fn transport(&self) -> &'a RtdReceipt {
        self.transport
    }
    pub fn declaration(&self) -> &Claim<RecoveryDeclaration> {
        &self.declaration
    }
    pub fn observations(&self) -> &[Claim<TailObservation>] {
        &self.observations
    }
    pub fn claim(&self) -> &Claim<RecoveryComparison> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn recovery(tail: f64, incoming: f64) -> Result<Option<f64>> {
    if incoming == 0.0 {
        Ok(None)
    } else {
        Ok(Some(quotient(sum([incoming, -tail])?, incoming)?))
    }
}

/// Compare a complete tail series against independently transported feed metal.
/// The caller declares the separation hypothesis; the operation never fits it.
pub fn compare_recovery<'a>(
    transport: &'a RtdReceipt,
    declaration: &Claim<RecoveryDeclaration>,
    tails: &[Claim<TailObservation>],
) -> Result<RecoveryReceipt<'a>> {
    admit_source(
        declaration,
        DECLARATION_METRIC,
        &declaration.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let d = &declaration.value;
    if d.transport_claim != transport.claim().claim_id
        || !transport.model().value.elements.contains(&d.element)
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "recovery declaration must bind this exact transport and element",
        ));
    }
    if !d.delay_then_separation
        || !d.complete_tail_population
        || !d.all_non_tail_element_is_recovered
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare delay then separation and complete non-tail recovery accounting",
        ));
    }
    if tails.len() != transport.summary().intervals.len() {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "one tail observation is required for every transport interval",
        ));
    }
    let mut tails = tails.to_vec();
    tails.sort_by_key(|t| {
        (
            t.value.scope.interval.start_ns,
            t.value.scope.interval.end_ns,
        )
    });
    let channel = MaterialChannel::Element(d.element.clone());
    let allowance =
        transport.model().value.closure_tolerances_kg[&channel].value() / (4 * tails.len()) as f64;
    let mut metadata = Metadata::new();
    metadata.include(transport.claim())?;
    metadata.include(declaration)?;
    let mut enc = Encoder::new_frame(*b"RTDC", 1);
    enc.put_str(transport.digest());
    put_source_envelope(&mut enc, declaration, 0);
    enc.put_u64(tails.len() as u64);
    let mut intervals = Vec::with_capacity(tails.len());
    for (observation, predicted) in tails.iter().zip(&transport.summary().intervals) {
        admit_source(
            observation,
            TAIL_METRIC,
            &observation.value.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?;
        let t = &observation.value;
        if t.scope != predicted.scope || t.element != d.element {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "tail support or elemental measurand differs from its transport interval",
            ));
        }
        let tail = nominal(t.tail_element_kg, allowance)?;
        let aligned = predicted.total_outflow_kg[&channel].value();
        let feed = predicted.feed_kg[&channel].value();
        if tail > aligned {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "observed tail exceeds aligned feed-equivalent metal under the declared separation hypothesis",
            ));
        }
        let point = IntervalRecovery {
            interval: t.scope.interval,
            tail_source: observation.claim_id,
            tail_element_kg: Kilograms::new(tail),
            same_interval_feed_element_kg: Kilograms::new(feed),
            aligned_feed_equivalent_kg: Kilograms::new(aligned),
            same_interval_recovery: recovery(tail, feed)?,
            lag_aligned_recovery: recovery(tail, aligned)?,
        };
        metadata.include(observation)?;
        put_source_envelope(&mut enc, observation, 1);
        for value in [tail, feed, aligned] {
            enc.put_u64(value.to_bits());
        }
        for value in [point.same_interval_recovery, point.lag_aligned_recovery] {
            enc.put_u8(u8::from(value.is_some()));
            if let Some(value) = value {
                enc.put_u64(value.to_bits());
            }
        }
        intervals.push(point);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = metadata.into_claim(
        OUTPUT_METRIC,
        RecoveryComparison {
            scope: transport.summary().scope.clone(),
            element: d.element.clone(),
            intervals,
        },
        &digest,
    );
    Ok(RecoveryReceipt {
        transport,
        declaration: declaration.clone(),
        observations: tails,
        claim,
        digest,
    })
}
