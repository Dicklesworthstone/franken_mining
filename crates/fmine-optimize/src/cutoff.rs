//! DESIGN (BD-252, plan §15.6): Lane's single-product limiting cut-offs.
//! Determinism: BitwisePortable

use std::cmp::Ordering;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;

/// One declared period on one project basis. All money uses the same currency,
/// all recovered and contained metal uses the same mass unit, and all ore
/// tonnage is dry metric tonnes. Recovery is constant across feed grades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneInputs {
    pub price_per_metal_unit: Rat,
    pub selling_cost_per_metal_unit: Rat,
    pub recovery_fraction: Rat,
    pub milling_cost_per_dry_tonne: Rat,
    pub fixed_cost_per_period: Rat,
    pub opportunity_cost_per_period: Rat,
    pub mill_dry_tonnes_per_period: Rat,
    pub market_metal_units_per_period: Rat,
}

/// Contained metal units per dry metric tonne under each limiting assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneCutoffs {
    pub mine: Rat,
    pub mill: Rat,
    pub market: Rat,
}

/// Comparison under the explicit rule that equality clears the threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradeScreen {
    EntireBandClears,
    EntireBandBelow,
    Straddles,
}

fn domain(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn arithmetic() -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        "Lane scenario exceeds exact rational arithmetic range",
    )
}

impl LaneInputs {
    /// Evaluate all three limiting grades as exact ratios.
    ///
    /// # Errors
    /// Refuses undeclared economic viability (nonpositive net margins), invalid
    /// recovery, capacities or costs, and arithmetic outside the exact range.
    pub fn evaluate(self) -> Result<LaneCutoffs, Refusal> {
        for (name, value) in [
            ("selling cost", self.selling_cost_per_metal_unit),
            ("milling cost", self.milling_cost_per_dry_tonne),
            ("fixed cost", self.fixed_cost_per_period),
            ("opportunity cost", self.opportunity_cost_per_period),
        ] {
            if value.signum() < 0 {
                return Err(domain(&format!("{name} must be nonnegative")));
            }
        }
        let recovery = self.recovery_fraction;
        if recovery.signum() <= 0 || recovery.num() > recovery.den() {
            return Err(domain("recovery must be a fraction in (0, 1]"));
        }
        if self.mill_dry_tonnes_per_period.signum() <= 0
            || self.market_metal_units_per_period.signum() <= 0
        {
            return Err(domain("mill and market capacities must be positive"));
        }
        let margin = self
            .price_per_metal_unit
            .checked_sub(self.selling_cost_per_metal_unit)
            .map_err(|_| arithmetic())?;
        if margin.signum() <= 0 {
            return Err(domain("metal price must exceed selling cost"));
        }
        let overhead = self
            .fixed_cost_per_period
            .checked_add(self.opportunity_cost_per_period)
            .map_err(|_| arithmetic())?;
        let market_margin = overhead
            .checked_div(self.market_metal_units_per_period)
            .and_then(|charge| margin.checked_sub(charge))
            .map_err(|_| arithmetic())?;
        if market_margin.signum() <= 0 {
            return Err(domain(
                "market-limited net metal margin must be positive after period charges",
            ));
        }
        let cost = self.milling_cost_per_dry_tonne;
        let mine_margin = margin.checked_mul(recovery).map_err(|_| arithmetic())?;
        let mill_cost = overhead
            .checked_div(self.mill_dry_tonnes_per_period)
            .and_then(|charge| cost.checked_add(charge))
            .map_err(|_| arithmetic())?;
        let market_margin = market_margin
            .checked_mul(recovery)
            .map_err(|_| arithmetic())?;
        Ok(LaneCutoffs {
            mine: cost.checked_div(mine_margin).map_err(|_| arithmetic())?,
            mill: mill_cost
                .checked_div(mine_margin)
                .map_err(|_| arithmetic())?,
            market: cost.checked_div(market_margin).map_err(|_| arithmetic())?,
        })
    }
}

/// Compare a closed feed-grade band with a threshold on the same support.
///
/// # Errors
/// Refuses invalid bounds or arithmetic outside the exact range.
pub fn screen_grade(lower: Rat, upper: Rat, threshold: Rat) -> Result<GradeScreen, Refusal> {
    let compare = |left: Rat, right| left.checked_cmp(right).map_err(|_| arithmetic());
    if lower.signum() < 0 || compare(lower, upper)? == Ordering::Greater {
        return Err(domain("feed-grade band requires 0 <= lower <= upper"));
    }
    Ok(if compare(lower, threshold)? != Ordering::Less {
        GradeScreen::EntireBandClears
    } else if compare(upper, threshold)? == Ordering::Less {
        GradeScreen::EntireBandBelow
    } else {
        GradeScreen::Straddles
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qty(s: &str) -> Rat {
        Rat::from_decimal_str(s).unwrap()
    }

    fn example() -> LaneInputs {
        LaneInputs {
            price_per_metal_unit: qty("100"),
            selling_cost_per_metal_unit: qty("5"),
            recovery_fraction: qty("0.9"),
            milling_cost_per_dry_tonne: qty("17.1"),
            fixed_cost_per_period: qty("30"),
            opportunity_cost_per_period: qty("27"),
            mill_dry_tonnes_per_period: qty("10"),
            market_metal_units_per_period: qty("7.5"),
        }
    }

    #[test]
    fn limit_grades_match_pinned_lane_values_and_price_response() {
        let inputs = example();
        let baseline = inputs.evaluate().unwrap();
        assert_eq!(baseline.mine, Rat::new(1, 5).unwrap());
        assert_eq!(baseline.mill, Rat::new(4, 15).unwrap());
        assert_eq!(baseline.market, Rat::new(5, 23).unwrap());
        let higher_price = LaneInputs {
            price_per_metal_unit: qty("110"),
            ..inputs
        }
        .evaluate()
        .unwrap();
        for (high, base) in [
            (higher_price.mine, baseline.mine),
            (higher_price.mill, baseline.mill),
            (higher_price.market, baseline.market),
        ] {
            assert_eq!(high.checked_cmp(base).unwrap(), Ordering::Less);
        }
        let no_overhead = LaneInputs {
            fixed_cost_per_period: qty("0"),
            opportunity_cost_per_period: qty("0"),
            ..inputs
        }
        .evaluate()
        .unwrap();
        assert_eq!(no_overhead.mill, baseline.mine);
        assert_eq!(no_overhead.market, baseline.mine);
    }

    #[test]
    fn poles_invalid_recovery_and_exact_range_refuse() {
        let inputs = example();
        for input in [
            LaneInputs {
                price_per_metal_unit: qty("5"),
                ..inputs
            },
            LaneInputs {
                market_metal_units_per_period: qty("0.6"),
                ..inputs
            },
            LaneInputs {
                milling_cost_per_dry_tonne: qty("-1"),
                ..inputs
            },
            LaneInputs {
                recovery_fraction: qty("1.01"),
                ..inputs
            },
            LaneInputs {
                recovery_fraction: qty("0"),
                ..inputs
            },
            LaneInputs {
                mill_dry_tonnes_per_period: qty("0"),
                ..inputs
            },
        ] {
            assert_eq!(input.evaluate().unwrap_err().code, "FMINE-REF-109");
        }
        let overflow = LaneInputs {
            fixed_cost_per_period: Rat::from_integer(i128::MAX),
            ..inputs
        };
        assert_eq!(overflow.evaluate().unwrap_err().code, "FMINE-REF-207");
    }

    #[test]
    fn feed_screen_includes_exact_equality_and_rejects_reversed_bands() {
        let threshold = qty("0.2");
        for (lower, upper, expected) in [
            ("0.2", "0.2", GradeScreen::EntireBandClears),
            ("0.1", "0.19", GradeScreen::EntireBandBelow),
            ("0.1", "0.2", GradeScreen::Straddles),
        ] {
            assert_eq!(
                screen_grade(qty(lower), qty(upper), threshold).unwrap(),
                expected
            );
        }
        assert_eq!(
            screen_grade(qty("0.3"), qty("0.2"), threshold)
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
    }
}
