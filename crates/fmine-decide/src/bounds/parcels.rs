//! DESIGN (BD-351). Whole-lot holdbacks on one additive route, with no replacement feed.
//! Determinism: Exact

use std::collections::BTreeMap;

use super::{
    Interval, RecoveryBoundModel, Result, Stability, ZERO, bad, contained_grams, exact,
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
}

pub(super) fn compare(
    lots: &BTreeMap<String, FeedParcel>,
    model: &RecoveryBoundModel,
) -> Result<(Interval, Vec<FeedCase>)> {
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
    }
    Ok((weights, cases))
}
