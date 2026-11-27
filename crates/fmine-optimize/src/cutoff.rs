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

/// Lane economics in one currency and one period. Recovery is supplied
/// separately as a constant fraction or a grade-dependent response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneTerms {
    pub price_per_metal_unit: Rat,
    pub selling_cost_per_metal_unit: Rat,
    pub milling_cost_per_dry_tonne: Rat,
    pub fixed_cost_per_period: Rat,
    pub opportunity_cost_per_period: Rat,
    pub mill_dry_tonnes_per_period: Rat,
    pub market_metal_units_per_period: Rat,
}

/// Contained metal units per dry metric tonne under each limiting assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneCutoffs<T = Rat> {
    pub mine: T,
    pub mill: T,
    pub market: T,
}

/// Economic crossing over the closed support of a recovery curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveCutoff {
    AtGrade(Rat),
    AllSupportedGradesClear,
    NoSupportedGradeClears,
}

/// Bound on the number of declared knots for one response.
pub const MAX_RECOVERY_KNOTS: usize = 128;

/// A monotone piecewise-linear curve of recovered metal per dry tonne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCurve {
    knots: Vec<RecoveryKnot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecoveryKnot {
    grade: Rat,
    recovered: Rat,
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

fn compare(left: Rat, right: Rat) -> Result<Ordering, Refusal> {
    left.checked_cmp(right).map_err(|_| arithmetic())
}

impl RecoveryCurve {
    /// Convert (contained grade, recovery fraction) knots into recovered grade.
    /// Interpolation uses recovered grade so a segment remains mass-conserving.
    ///
    /// # Errors
    /// Refuses nonphysical knots, non-increasing contained grades, decreasing
    /// recovered grades, unsupported knot counts and exact arithmetic overflow.
    pub fn new(points: &[(Rat, Rat)], max_grade: Rat) -> Result<Self, Refusal> {
        if !(2..=MAX_RECOVERY_KNOTS).contains(&points.len()) {
            return Err(domain("a recovery curve requires 2 to 128 knots"));
        }
        let mut knots: Vec<RecoveryKnot> = Vec::with_capacity(points.len());
        for &(grade, fraction) in points {
            if grade.signum() < 0 || compare(grade, max_grade)?.is_gt() {
                return Err(domain(
                    "recovery curve grades must fit within one dry tonne",
                ));
            }
            if fraction.signum() < 0 || fraction.num() > fraction.den() {
                return Err(domain("curve recovery fractions must be in [0, 1]"));
            }
            let recovered = grade.checked_mul(fraction).map_err(|_| arithmetic())?;
            if let Some(previous) = knots.last() {
                if !compare(grade, previous.grade)?.is_gt() {
                    return Err(domain("recovery curve grades must strictly increase"));
                }
                if compare(recovered, previous.recovered)?.is_lt() {
                    return Err(domain("recovered metal per dry tonne must not decrease"));
                }
            }
            knots.push(RecoveryKnot { grade, recovered });
        }
        Ok(Self { knots })
    }

    /// First and last admitted contained grades, both inclusive.
    pub fn support(&self) -> (Rat, Rat) {
        (self.knots[0].grade, self.knots[self.knots.len() - 1].grade)
    }

    /// Admit a feed band only if its entire support is covered by the curve.
    ///
    /// # Errors
    /// Refuses reversed or negative bands and any extrapolation.
    pub fn check_band(&self, lower: Rat, upper: Rat) -> Result<(), Refusal> {
        check_band(lower, upper)?;
        let (first, last) = self.support();
        if compare(lower, first)?.is_lt() || compare(upper, last)?.is_gt() {
            return Err(Refusal::new(
                RefusalCode::OutsideAoa203,
                "feed-grade band extends outside the recovery curve's declared grade range",
            ));
        }
        Ok(())
    }

    fn cutoff(&self, cost: Rat, margin: Rat) -> Result<CurveCutoff, Refusal> {
        let required = cost.checked_div(margin).map_err(|_| arithmetic())?;
        let first = self.knots[0];
        match compare(required, first.recovered)? {
            Ordering::Less => return Ok(CurveCutoff::AllSupportedGradesClear),
            Ordering::Equal => return Ok(CurveCutoff::AtGrade(first.grade)),
            Ordering::Greater => {}
        }
        for pair in self.knots.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if compare(required, right.recovered)?.is_gt() {
                continue;
            }
            // The first hit has a strictly positive rise even when subsequent
            // knots form a plateau. Equality returns its leftmost crossing.
            if required == right.recovered {
                return Ok(CurveCutoff::AtGrade(right.grade));
            }
            let grade = required
                .checked_sub(left.recovered)
                .and_then(|rise| rise.checked_div(right.recovered.checked_sub(left.recovered)?))
                .and_then(|part| part.checked_mul(right.grade.checked_sub(left.grade)?))
                .and_then(|offset| left.grade.checked_add(offset))
                .map_err(|_| arithmetic())?;
            return Ok(CurveCutoff::AtGrade(grade));
        }
        Ok(CurveCutoff::NoSupportedGradeClears)
    }
}

impl LaneInputs {
    /// Evaluate all three limiting grades as exact ratios.
    ///
    /// # Errors
    /// Refuses undeclared economic viability (nonpositive net margins), invalid
    /// recovery, capacities or costs, and arithmetic outside the exact range.
    pub fn evaluate(self) -> Result<LaneCutoffs, Refusal> {
        LaneTerms {
            price_per_metal_unit: self.price_per_metal_unit,
            selling_cost_per_metal_unit: self.selling_cost_per_metal_unit,
            milling_cost_per_dry_tonne: self.milling_cost_per_dry_tonne,
            fixed_cost_per_period: self.fixed_cost_per_period,
            opportunity_cost_per_period: self.opportunity_cost_per_period,
            mill_dry_tonnes_per_period: self.mill_dry_tonnes_per_period,
            market_metal_units_per_period: self.market_metal_units_per_period,
        }
        .evaluate_constant(self.recovery_fraction)
    }
}

impl LaneTerms {
    /// Evaluate with one fixed recovery fraction.
    ///
    /// # Errors
    /// Refuses invalid economics, out-of-domain recovery and exact overflow.
    pub fn evaluate_constant(self, recovery: Rat) -> Result<LaneCutoffs, Refusal> {
        if recovery.signum() <= 0 || recovery.num() > recovery.den() {
            return Err(domain("recovery must be a fraction in (0, 1]"));
        }
        self.map_limits(|cost, margin| {
            margin
                .checked_mul(recovery)
                .and_then(|denom| cost.checked_div(denom))
                .map_err(|_| arithmetic())
        })
    }

    /// Evaluate the limiting grades within the curve's declared support.
    ///
    /// # Errors
    /// Refuses invalid economics or exact overflow.
    pub fn evaluate_curve(
        self,
        curve: &RecoveryCurve,
    ) -> Result<LaneCutoffs<CurveCutoff>, Refusal> {
        self.map_limits(|cost, margin| curve.cutoff(cost, margin))
    }

    fn map_limits<T>(
        self,
        grade: impl Fn(Rat, Rat) -> Result<T, Refusal>,
    ) -> Result<LaneCutoffs<T>, Refusal> {
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
        let mill_cost = overhead
            .checked_div(self.mill_dry_tonnes_per_period)
            .and_then(|charge| cost.checked_add(charge))
            .map_err(|_| arithmetic())?;
        Ok(LaneCutoffs {
            mine: grade(cost, margin)?,
            mill: grade(mill_cost, margin)?,
            market: grade(cost, market_margin)?,
        })
    }
}

/// Compare a closed feed-grade band with a threshold on the same support.
///
/// # Errors
/// Refuses invalid bounds or arithmetic outside the exact range.
pub fn screen_grade(lower: Rat, upper: Rat, threshold: Rat) -> Result<GradeScreen, Refusal> {
    check_band(lower, upper)?;
    Ok(if compare(lower, threshold)? != Ordering::Less {
        GradeScreen::EntireBandClears
    } else if compare(upper, threshold)? == Ordering::Less {
        GradeScreen::EntireBandBelow
    } else {
        GradeScreen::Straddles
    })
}

fn check_band(lower: Rat, upper: Rat) -> Result<(), Refusal> {
    if lower.signum() < 0 || compare(lower, upper)?.is_gt() {
        return Err(domain("feed-grade band requires 0 <= lower <= upper"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qty(s: &str) -> Rat {
        Rat::from_decimal_str(s).unwrap()
    }

    #[test]
    fn curve_inverts_recovered_grade_and_keeps_the_first_plateau_crossing() {
        let curve = RecoveryCurve::new(
            &[
                (qty("0.2"), qty("0.5")),
                (qty("0.4"), qty("0.75")),
                (qty("0.6"), qty("0.5")),
            ],
            qty("1"),
        )
        .unwrap();
        let cases = [
            ("0.09", CurveCutoff::AllSupportedGradesClear),
            ("0.1", CurveCutoff::AtGrade(qty("0.2"))),
            ("0.2", CurveCutoff::AtGrade(qty("0.3"))),
            ("0.3", CurveCutoff::AtGrade(qty("0.4"))),
            ("0.31", CurveCutoff::NoSupportedGradeClears),
        ];
        for (needed, expected) in cases {
            assert_eq!(curve.cutoff(qty(needed), qty("1")).unwrap(), expected);
        }
        curve.check_band(qty("0.2"), qty("0.6")).unwrap();
        assert_eq!(
            curve.check_band(qty("0.3"), qty("0.61")).unwrap_err().code,
            "FMINE-REF-203"
        );
        assert_eq!(
            curve.check_band(qty("0.15"), qty("0.3")).unwrap_err().code,
            "FMINE-REF-203"
        );
    }

    #[test]
    fn a_constant_recovery_curve_matches_legacy_limiting_grades_within_support() {
        let curve = RecoveryCurve::new(
            &[(qty("0"), qty("0.75")), (qty("1"), qty("0.75"))],
            qty("1"),
        )
        .unwrap();
        let terms = LaneTerms {
            price_per_metal_unit: qty("100"),
            selling_cost_per_metal_unit: qty("0"),
            milling_cost_per_dry_tonne: qty("20"),
            fixed_cost_per_period: qty("10"),
            opportunity_cost_per_period: qty("10"),
            mill_dry_tonnes_per_period: qty("2"),
            market_metal_units_per_period: qty("2"),
        };
        let plain = terms.evaluate_constant(qty("0.75")).unwrap();
        let curved = terms.evaluate_curve(&curve).unwrap();
        assert_eq!(curved.mine, CurveCutoff::AtGrade(plain.mine));
        assert_eq!(curved.mill, CurveCutoff::AtGrade(plain.mill));
        assert_eq!(curved.market, CurveCutoff::AtGrade(plain.market));
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
