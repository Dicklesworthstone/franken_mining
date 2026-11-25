//! DESIGN (BD-351). Whole or proportional lot holdbacks on one additive route, with no replacement feed.
//! Determinism: Exact

use std::collections::BTreeMap;

use super::{
    Interval, ONE, Rat, RecoveryBoundModel, Result, Stability, ZERO, bad, contained_grams, exact,
    positive_metal_share, recovery_range, stability,
};
use fmine_evidence::{Refusal, RefusalCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Testwork {
    Tested,
    Untested,
}

impl Testwork {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tested => "tested",
            Self::Untested => "untested",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedParcel {
    pub testwork: Testwork,
    pub dry_tonnes: Interval,
    pub grade_g_per_t: Interval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedCase {
    pub held_back: Option<String>,
    pub dry_tonnes: Interval,
    pub contained_grams: Interval,
    pub recovered_grams: Interval,
    /// Full-feed production minus this case at the same mass, grade and recovery inputs.
    pub forgone_recovered_grams: Interval,
    pub recovery: std::result::Result<(Interval, Stability), Refusal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldbackLimit {
    TargetUnattainable,
    ZeroPossibleRetainedMetal,
}

impl HoldbackLimit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetUnattainable => "target_unattainable",
            Self::ZeroPossibleRetainedMetal => "zero_possible_retained_metal",
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::TargetUnattainable => "No fraction of this lot guarantees the recovery target.",
            Self::ZeroPossibleRetainedMetal => {
                "The target would require removing the entire lot, and the retained feed can contain zero metal."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialHoldback {
    pub lot: String,
    pub minimum: std::result::Result<MinimumHoldback, HoldbackLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimumHoldback {
    pub fraction: Rat,
    pub held_dry_tonnes: Interval,
    pub retained_feed: FeedCase,
}

pub(super) struct FeedComparison {
    pub weights: Interval,
    pub cases: Vec<FeedCase>,
    pub partial_holdback: Option<PartialHoldback>,
}

const EMPTY: Interval = Interval {
    lower: ZERO,
    upper: ZERO,
};

fn sum(a: Interval, b: Interval) -> Result<Interval> {
    Ok(Interval {
        lower: exact(a.lower.checked_add(b.lower))?,
        upper: exact(a.upper.checked_add(b.upper))?,
    })
}

// The subtrahend is the same lot that was added to these totals. Remove its contribution
// from each endpoint separately; ordinary subtraction of independent boxes would widen it.
fn without(total: Interval, lot: Interval) -> Result<Interval> {
    Ok(Interval {
        lower: exact(total.lower.checked_sub(lot.lower))?,
        upper: exact(total.upper.checked_sub(lot.upper))?,
    })
}

fn recovered(metal: Interval, fraction: Interval) -> Result<Interval> {
    Ok(Interval {
        lower: exact(metal.lower.checked_mul(fraction.lower))?,
        upper: exact(metal.upper.checked_mul(fraction.upper))?,
    })
}

fn scale(quantity: Interval, fraction: Rat) -> Result<Interval> {
    recovered(
        quantity,
        Interval {
            lower: fraction,
            upper: fraction,
        },
    )
}

#[derive(Clone, Copy)]
struct Totals {
    tonnes: Interval,
    tested: Interval,
    untested: Interval,
}

impl Totals {
    fn metal_mut(&mut self, role: Testwork) -> &mut Interval {
        match role {
            Testwork::Tested => &mut self.tested,
            Testwork::Untested => &mut self.untested,
        }
    }

    fn case(
        self,
        held_back: Option<String>,
        forgone_recovered_grams: Interval,
        model: &RecoveryBoundModel,
    ) -> Result<FeedCase> {
        let recovery = positive_metal_share(self.tested, self.untested)
            .and_then(|w| recovery_range(w, model))
            .and_then(|r| Ok((r, stability(r, model.threshold)?)));
        Ok(FeedCase {
            held_back,
            dry_tonnes: self.tonnes,
            contained_grams: sum(self.tested, self.untested)?,
            recovered_grams: sum(
                recovered(self.tested, model.tested_recovery)?,
                recovered(self.untested, model.untested_recovery)?,
            )?,
            forgone_recovered_grams,
            recovery,
        })
    }

    // With positive retained metal, the target holds iff the metal-weighted margin
    // is nonnegative. Each role uses its lower recovery and the adverse mass endpoint.
    fn worst_margin(self, model: &RecoveryBoundModel) -> Result<Rat> {
        let contribution = |metal: Interval, recovery: Interval| -> Result<Rat> {
            let coefficient = exact(recovery.lower.checked_sub(model.threshold))?;
            let adverse = if coefficient.num() < 0 {
                metal.upper
            } else {
                metal.lower
            };
            exact(coefficient.checked_mul(adverse))
        };
        exact(
            contribution(self.tested, model.tested_recovery)?
                .checked_add(contribution(self.untested, model.untested_recovery)?),
        )
    }
}

fn minimum_holdback(
    full: Totals,
    mut kept: Totals,
    parcel: &FeedParcel,
    metal: Interval,
    model: &RecoveryBoundModel,
) -> Result<std::result::Result<MinimumHoldback, HoldbackLimit>> {
    let full_margin = full.worst_margin(model)?;
    let fraction = if full_margin.num() >= 0 {
        ZERO
    } else {
        let removed_margin = kept.worst_margin(model)?;
        if removed_margin.num() < 0 {
            return Ok(Err(HoldbackLimit::TargetUnattainable));
        }
        // At fixed grade each mass endpoint scales with the retained fraction. The
        // adverse margin is affine between full feed and removal of the named lot.
        exact(full_margin.checked_div(exact(full_margin.checked_sub(removed_margin))?))?
    };
    let retained = exact(ONE.checked_sub(fraction))?;
    kept.tonnes = sum(kept.tonnes, scale(parcel.dry_tonnes, retained)?)?;
    let bin = kept.metal_mut(parcel.testwork);
    *bin = sum(*bin, scale(metal, retained)?)?;
    if kept.tested.lower == ZERO && kept.untested.lower == ZERO {
        return Ok(Err(HoldbackLimit::ZeroPossibleRetainedMetal));
    }
    let recovery = match parcel.testwork {
        Testwork::Tested => model.tested_recovery,
        Testwork::Untested => model.untested_recovery,
    };
    let result = kept.case(None, recovered(scale(metal, fraction)?, recovery)?, model)?;
    let (bounds, verdict) = result.recovery.clone()?;
    if verdict != Stability::StableProceed || (fraction != ZERO && bounds.lower != model.threshold)
    {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "the retained feed does not reproduce the exact minimum holdback boundary",
        ));
    }
    Ok(Ok(MinimumHoldback {
        fraction,
        held_dry_tonnes: scale(parcel.dry_tonnes, fraction)?,
        retained_feed: result,
    }))
}

pub(super) fn compare(
    lots: &BTreeMap<String, FeedParcel>,
    model: &RecoveryBoundModel,
) -> Result<FeedComparison> {
    if let Some(id) = &model.partial_holdback
        && !lots.contains_key(id)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            format!("partial holdback names an unknown feed lot {id:?}"),
        ));
    }
    let mut total = Totals {
        tonnes: EMPTY,
        tested: EMPTY,
        untested: EMPTY,
    };
    let mut contributions = Vec::with_capacity(lots.len());
    for (id, parcel) in lots {
        if id.is_empty() || id.trim() != id || id.chars().any(char::is_control) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "each feed lot needs a distinct nonempty identifier",
            ));
        }
        let metal = contained_grams(parcel.dry_tonnes, parcel.grade_g_per_t)?;
        total.tonnes = sum(total.tonnes, parcel.dry_tonnes)?;
        let bin = total.metal_mut(parcel.testwork);
        *bin = sum(*bin, metal)?;
        contributions.push((id, parcel, metal));
    }
    let weights = positive_metal_share(total.tested, total.untested)?;
    let mut cases = Vec::with_capacity(lots.len() + 1);
    let mut partial_holdback = None;
    cases.push(total.case(None, EMPTY, model)?);
    for (id, parcel, metal) in contributions {
        let mut kept = total;
        kept.tonnes = without(kept.tonnes, parcel.dry_tonnes)?;
        let bin = kept.metal_mut(parcel.testwork);
        *bin = without(*bin, metal)?;
        let lot_recovery = match parcel.testwork {
            Testwork::Tested => model.tested_recovery,
            Testwork::Untested => model.untested_recovery,
        };
        cases.push(kept.case(Some(id.clone()), recovered(metal, lot_recovery)?, model)?);
        if model.partial_holdback.as_ref() == Some(id) {
            partial_holdback = Some(PartialHoldback {
                lot: id.clone(),
                minimum: minimum_holdback(total, kept, parcel, metal, model)?,
            });
        }
    }
    Ok(FeedComparison {
        weights,
        cases,
        partial_holdback,
    })
}
