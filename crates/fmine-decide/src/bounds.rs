//! DESIGN: exact two-stratum recovery decision bounds (BD-351.S1, plan §17.4).
//! These are conditional identification bounds, not measured recovery or probabilities.
//! Determinism: Exact (rational endpoints; inherited UU metadata is bound by its exact bits).

use fmine_claim::{AdditivityClass, Claim, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_numerics::exact_feas::Rat;
use fmine_units::UnitsError;

pub mod model;
pub mod parcels;
mod render;
#[cfg(test)]
mod tests;

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
pub const INPUT_METRIC: &str = "decision.untested-ore.inputs/1";
pub const OUTPUT_METRIC: &str = "decision.untested-ore.bounds/1";
pub const NO_CLAIM: &str = "Decision support conditional on the declared two-stratum additive recovery model and exact input bounds. Bounds are not probabilities, measured recovery, calibrated predictions, physical feasibility or evidence that untested ore behaves like tested ore. Testing conditions narrow the declared untested-recovery interval only; no likelihood, VOI price, reserve classification, disclosure authority or execution authorization is supplied.";

/// Closed exact interval. Physical domains are checked by the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub lower: Rat,
    pub upper: Rat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShareRange {
    interval: Interval,
    lower_inclusive: bool,
    upper_inclusive: bool,
}

type ShareWindows = (Option<ShareRange>, Option<ShareRange>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeightBasis {
    /// Fraction of the combined contained metal, never a tonne fraction.
    ContainedMetal { untested_fraction: Interval },
    /// Independent closed intervals in dry metric tonnes and grams per dry metric tonne.
    DryTonnesAndGrade {
        tested_tonnes: Interval,
        tested_grade_g_per_t: Interval,
        untested_tonnes: Interval,
        untested_grade_g_per_t: Interval,
    },
    /// Named, disjoint lots in the same feed period, route and metal.
    Parcels(std::collections::BTreeMap<String, parcels::FeedParcel>),
}

/// One explicitly sourced model; its Claim class must describe its weakest declaration.
/// The worksheet parser admits declarations as Assumed, without inferring testwork evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryBoundModel {
    pub model_key: String,
    pub question: String,
    pub route: String,
    pub metal: String,
    pub additivity_basis: String,
    pub untested_bound_basis: String,
    pub weights: WeightBasis,
    pub tested_recovery: Interval,
    pub untested_recovery: Interval,
    /// Proceed iff aggregate recovery is at least this fraction; stop is strictly below it.
    pub threshold: Rat,
    /// Any declared interaction makes additive averaging inadmissible.
    pub interactions: Vec<String>,
}

impl RecoveryBoundModel {
    pub fn canonical_tag(&self) -> String {
        format!(
            "untested-ore-inputs/v1:{}",
            sha256(render::model_json(self).as_bytes())
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stability {
    StableProceed,
    StableStop,
    Unstable,
}
impl Stability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StableProceed => "stable_proceed",
            Self::StableStop => "stable_stop",
            Self::Unstable => "unstable",
        }
    }
}

/// A condition attainable by narrowing the admitted untested interval, with all other
/// inputs unchanged. An impossible condition never invents a test outside that interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestCondition {
    AlreadyStable,
    RecoveryLowerAtLeast(Rat),
    RecoveryUpperBelow(Rat),
    Impossible { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vertex {
    pub untested_metal_fraction: Rat,
    pub tested_recovery: Rat,
    pub untested_recovery: Rat,
    pub aggregate_recovery: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundSummary {
    pub untested_metal_fraction: Interval,
    pub recovery: Interval,
    pub threshold: Rat,
    pub stability: Stability,
    pub proceed_condition: TestCondition,
    pub stop_condition: TestCondition,
    pub vertices: Vec<Vertex>,
    /// All feed, then each whole-lot holdback. Empty for aggregate weight declarations.
    pub parcel_cases: Vec<parcels::FeedCase>,
}

/// Immutable source/model/output snapshot; the digest also binds metadata not in generic Claim ids.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundCard {
    inputs: Claim<RecoveryBoundModel>,
    claim: Claim<BoundSummary>,
    digest: String,
}
impl BoundCard {
    pub fn inputs(&self) -> &Claim<RecoveryBoundModel> {
        &self.inputs
    }
    pub fn claim(&self) -> &Claim<BoundSummary> {
        &self.claim
    }
    pub fn summary(&self) -> &BoundSummary {
        &self.claim.value
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, why: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("untested-ore bounds: {}", why.into()))
}
fn exact<T>(result: std::result::Result<T, UnitsError>) -> Result<T> {
    result.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact arithmetic is unrepresentable",
        )
    })
}
fn min(a: Rat, b: Rat) -> Result<Rat> {
    Ok(if exact(a.checked_cmp(b))?.is_gt() {
        b
    } else {
        a
    })
}
fn max(a: Rat, b: Rat) -> Result<Rat> {
    Ok(if exact(a.checked_cmp(b))?.is_lt() {
        b
    } else {
        a
    })
}
fn interval(value: Interval, fraction: bool) -> Result<()> {
    if value.lower.signum() < 0
        || exact(value.lower.checked_cmp(value.upper))?.is_gt()
        || (fraction && exact(value.upper.checked_cmp(ONE))?.is_gt())
    {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            if fraction {
                "fraction intervals must satisfy 0 <= lower <= upper <= 1"
            } else {
                "dry tonnes and grades need ordered nonnegative intervals"
            },
        ));
    }
    Ok(())
}
fn contained_grams(tonnes: Interval, grade_g_per_t: Interval) -> Result<Interval> {
    interval(tonnes, false)?;
    interval(grade_g_per_t, false)?;
    if exact(
        grade_g_per_t
            .upper
            .checked_cmp(Rat::from_integer(1_000_000)),
    )?
    .is_gt()
    {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "contained metal grade exceeds 1,000,000 g per dry tonne",
        ));
    }
    Ok(Interval {
        lower: exact(tonnes.lower.checked_mul(grade_g_per_t.lower))?,
        upper: exact(tonnes.upper.checked_mul(grade_g_per_t.upper))?,
    })
}

/// Extremes of U/(T+U) on two independent nonnegative mass intervals.
fn untested_share(tested: Interval, untested: Interval) -> Result<Interval> {
    Ok(Interval {
        lower: exact(
            untested
                .lower
                .checked_div(exact(untested.lower.checked_add(tested.upper))?),
        )?,
        upper: exact(
            untested
                .upper
                .checked_div(exact(untested.upper.checked_add(tested.lower))?),
        )?,
    })
}

fn weights_and_cases(model: &RecoveryBoundModel) -> Result<(Interval, Vec<parcels::FeedCase>)> {
    let result = match &model.weights {
        WeightBasis::Parcels(lots) => return parcels::compare(lots, model),
        WeightBasis::ContainedMetal { untested_fraction } => *untested_fraction,
        WeightBasis::DryTonnesAndGrade {
            tested_tonnes,
            tested_grade_g_per_t,
            untested_tonnes,
            untested_grade_g_per_t,
        } => {
            let tested = contained_grams(*tested_tonnes, *tested_grade_g_per_t)?;
            let untested = contained_grams(*untested_tonnes, *untested_grade_g_per_t)?;
            positive_metal_share(tested, untested)?
        }
    };
    interval(result, true)?;
    Ok((result, Vec::new()))
}

fn positive_metal_share(tested: Interval, untested: Interval) -> Result<Interval> {
    if tested.lower == ZERO && untested.lower == ZERO {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "the admitted tonne/grade box includes zero combined contained metal",
        ));
    }
    untested_share(tested, untested)
}
fn aggregate(weight: Rat, tested: Rat, untested: Rat) -> Result<Rat> {
    exact(
        exact(exact(ONE.checked_sub(weight))?.checked_mul(tested))?
            .checked_add(exact(weight.checked_mul(untested))?),
    )
}

fn recovery_range(weights: Interval, model: &RecoveryBoundModel) -> Result<Interval> {
    let t = model.tested_recovery;
    let u = model.untested_recovery;
    Ok(Interval {
        lower: min(
            aggregate(weights.lower, t.lower, u.lower)?,
            aggregate(weights.upper, t.lower, u.lower)?,
        )?,
        upper: max(
            aggregate(weights.lower, t.upper, u.upper)?,
            aggregate(weights.upper, t.upper, u.upper)?,
        )?,
    })
}

fn stability(range: Interval, threshold: Rat) -> Result<Stability> {
    Ok(if !exact(range.lower.checked_cmp(threshold))?.is_lt() {
        Stability::StableProceed
    } else if exact(range.upper.checked_cmp(threshold))?.is_lt() {
        Stability::StableStop
    } else {
        Stability::Unstable
    })
}

/// At fixed share the worst and best recoveries use paired lower and upper endpoints,
/// respectively. Each is affine in share, so a nonconstant line crosses the cutoff at most once.
fn share_range(
    weights: Interval,
    tested: Rat,
    untested: Rat,
    cutoff: Rat,
    proceed: bool,
) -> Result<Option<ShareRange>> {
    let admits = |w| -> Result<bool> {
        let below = exact(aggregate(w, tested, untested)?.checked_cmp(cutoff))?.is_lt();
        Ok(if proceed { !below } else { below })
    };
    let left = admits(weights.lower)?;
    let right = admits(weights.upper)?;
    if left == right {
        return Ok(left.then_some(ShareRange {
            interval: weights,
            lower_inclusive: true,
            upper_inclusive: true,
        }));
    }
    let slope = exact(untested.checked_sub(tested))?;
    let crossing = exact(exact(cutoff.checked_sub(tested))?.checked_div(slope))?;
    Ok(Some(ShareRange {
        interval: Interval {
            lower: if left { weights.lower } else { crossing },
            upper: if right { weights.upper } else { crossing },
        },
        lower_inclusive: left || proceed,
        upper_inclusive: right || proceed,
    }))
}

impl BoundCard {
    fn share_ranges(&self) -> Result<ShareWindows> {
        let w = self.summary().untested_metal_fraction;
        let m = &self.inputs().value;
        Ok((
            share_range(
                w,
                m.tested_recovery.lower,
                m.untested_recovery.lower,
                m.threshold,
                true,
            )?,
            share_range(
                w,
                m.tested_recovery.upper,
                m.untested_recovery.upper,
                m.threshold,
                false,
            )?,
        ))
    }

    fn dry_share_ranges(&self) -> Result<Option<ShareWindows>> {
        let m = &self.inputs().value;
        let WeightBasis::DryTonnesAndGrade {
            tested_tonnes,
            tested_grade_g_per_t,
            untested_tonnes,
            untested_grade_g_per_t,
        } = m.weights
        else {
            return Ok(None);
        };
        let dry = untested_share(tested_tonnes, untested_tonnes)?;
        // At dry share p the threshold is met iff the mass-weighted sum of
        // g*(recovery-threshold) is nonnegative. Total contained metal is positive
        // everywhere in the admitted box. This also handles a zero-grade stratum.
        let window = |proceed| {
            let margin = |grades: Interval, recoveries: Interval| {
                let r = if proceed {
                    recoveries.lower
                } else {
                    recoveries.upper
                };
                let delta = exact(r.checked_sub(m.threshold))?;
                let a = exact(grades.lower.checked_mul(delta))?;
                let b = exact(grades.upper.checked_mul(delta))?;
                if proceed { min(a, b) } else { max(a, b) }
            };
            share_range(
                dry,
                margin(tested_grade_g_per_t, m.tested_recovery)?,
                margin(untested_grade_g_per_t, m.untested_recovery)?,
                ZERO,
                proceed,
            )
        };
        Ok(Some((window(true)?, window(false)?)))
    }
}

/// Solve both endpoint inequalities. At zero untested weight, the tested stratum alone
/// controls the constraint, so no division or fictitious decisive test is permitted.
fn condition(
    weights: Interval,
    tested: Rat,
    untested: Interval,
    threshold: Rat,
    proceed: bool,
) -> Result<TestCondition> {
    let impossible = |reason: &str| TestCondition::Impossible {
        reason: reason.into(),
    };
    let mut required = None;
    for weight in [weights.lower, weights.upper] {
        if weight == ZERO {
            let comparison = exact(tested.checked_cmp(threshold))?;
            if (proceed && comparison.is_lt()) || (!proceed && !comparison.is_lt()) {
                return Ok(impossible(
                    "the tested-only endpoint cannot reach this action by testing untested ore",
                ));
            }
            continue;
        }
        let tested_part = exact(exact(ONE.checked_sub(weight))?.checked_mul(tested))?;
        let boundary = exact(exact(threshold.checked_sub(tested_part))?.checked_div(weight))?;
        required = Some(match required {
            None => boundary,
            Some(old) if proceed => max(old, boundary)?,
            Some(old) => min(old, boundary)?,
        });
    }
    let Some(boundary) = required else {
        return Ok(TestCondition::AlreadyStable);
    };
    if proceed {
        if !exact(untested.lower.checked_cmp(boundary))?.is_lt() {
            Ok(TestCondition::AlreadyStable)
        } else if exact(untested.upper.checked_cmp(boundary))?.is_lt() {
            Ok(impossible(
                "no narrowing of the admitted untested interval can guarantee proceed",
            ))
        } else {
            Ok(TestCondition::RecoveryLowerAtLeast(boundary))
        }
    } else if exact(untested.upper.checked_cmp(boundary))?.is_lt() {
        Ok(TestCondition::AlreadyStable)
    } else if !exact(untested.lower.checked_cmp(boundary))?.is_lt() {
        Ok(impossible(
            "no narrowing of the admitted untested interval can guarantee stop",
        ))
    } else {
        Ok(TestCondition::RecoveryUpperBelow(boundary))
    }
}

/// Evaluate every endpoint combination, retain witnesses, and check the closed-form
/// extrema. All intervals form an independent box; no unstated dependence is inferred.
pub fn evaluate(inputs: &Claim<RecoveryBoundModel>) -> Result<BoundCard> {
    crate::card::admit(
        inputs,
        &inputs.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if inputs.metric_key != INPUT_METRIC {
        return Err(bad(RefusalCode::BasisMismatch105, "wrong input metric"));
    }
    if inputs.uu_terms.len() != 1 {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "declare exactly one UU scenario for the untested-stratum bound",
        ));
    }
    let m = &inputs.value;
    for name in [
        &m.model_key,
        &m.question,
        &m.route,
        &m.metal,
        &m.additivity_basis,
        &m.untested_bound_basis,
    ] {
        if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "model, route, metal and bound/additivity bases must be named",
            ));
        }
    }
    if !m.interactions.is_empty() {
        return Err(bad(
            RefusalCode::NonAdditiveAverage103,
            "aggregate unresolved: declared blending interactions invalidate additive parcel recoveries",
        ));
    }
    interval(m.tested_recovery, true)?;
    interval(m.untested_recovery, true)?;
    interval(
        Interval {
            lower: m.threshold,
            upper: m.threshold,
        },
        true,
    )?;
    let (weights, parcel_cases) = weights_and_cases(m)?;
    let mut vertices = Vec::with_capacity(8);
    let (mut lower, mut upper) = (ONE, ZERO);
    for w in [weights.lower, weights.upper] {
        for tested in [m.tested_recovery.lower, m.tested_recovery.upper] {
            for untested in [m.untested_recovery.lower, m.untested_recovery.upper] {
                let value = aggregate(w, tested, untested)?;
                lower = min(lower, value)?;
                upper = max(upper, value)?;
                vertices.push(Vertex {
                    untested_metal_fraction: w,
                    tested_recovery: tested,
                    untested_recovery: untested,
                    aggregate_recovery: value,
                });
            }
        }
    }
    let recovery = recovery_range(weights, m)?;
    if lower != recovery.lower || upper != recovery.upper {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "vertex and closed-form extrema disagree",
        ));
    }
    let summary = BoundSummary {
        untested_metal_fraction: weights,
        recovery,
        threshold: m.threshold,
        stability: stability(recovery, m.threshold)?,
        proceed_condition: condition(
            weights,
            m.tested_recovery.lower,
            m.untested_recovery,
            m.threshold,
            true,
        )?,
        stop_condition: condition(
            weights,
            m.tested_recovery.upper,
            m.untested_recovery,
            m.threshold,
            false,
        )?,
        vertices,
        parcel_cases,
    };
    let mut binding = Encoder::new_frame(*b"UBND", 1);
    binding.put_str(&crate::card::render::claim_json(
        inputs,
        &render::model_json(m),
    ));
    binding.put_str(&render::summary_json(&summary));
    let digest = sha256(&binding.finish()).to_string();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &format!("untested-ore-bounds/v1:{digest}"),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        inputs.evidence.meet(EvidenceClass::ModelledUncalibrated),
        inputs
            .epistemic
            .and(EpistemicStatus::KnownUnknown(KuKind::Parametric)),
        KnowledgeState::Estimated,
        inputs.confidentiality.clone(),
        inputs.uu_terms.clone(),
        vec![inputs.claim_id],
        inputs.anchors.clone(),
    );
    Ok(BoundCard {
        inputs: inputs.clone(),
        claim,
        digest,
    })
}
