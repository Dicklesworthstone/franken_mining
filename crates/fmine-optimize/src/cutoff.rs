//! DESIGN (BD-252, plan §15.6): Lane's single-product limiting cut-offs.
//! Determinism: BitwisePortable

use std::cmp::Ordering;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;

mod blend;
pub mod routing;
pub use blend::{BlendAllocation, BlendConstraints, EnergyLimit, EnergyUse, ImpurityLimit};

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
/// Bound on the grade-tonnage population of one capacity screen.
pub const MAX_GRADE_BINS: usize = 128;

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

/// One grade population on the declared period and dry-tonne basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradeTonnageBin {
    pub grade: Rat,
    pub dry_tonnes: Rat,
}

/// Recovery assumption already admitted by the Lane scenario.
#[derive(Debug, Clone, Copy)]
pub enum GradeRecovery<'a> {
    Constant(Rat),
    Curve(&'a RecoveryCurve),
}

/// Lowest grade selected and the share of its dry tonnes that fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradeBoundary {
    pub grade: Rat,
    pub fraction: Rat,
}

/// Portion of the highest-grade-first feed that fits this cap or cap pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradeAllocation {
    pub dry_tonnes: Rat,
    pub recovered_metal: Rat,
    pub boundary: Option<GradeBoundary>,
}

/// Physical capacity and positive-contribution screens on the same population.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityBalance {
    pub available_dry_tonnes: Rat,
    pub available_recovered_metal: Rat,
    pub mill_only: GradeAllocation,
    pub market_only: GradeAllocation,
    pub both: GradeAllocation,
    pub positive_contribution_feed: GradeAllocation,
    /// Recovered metal at net selling price less variable milling cost, in the declared currency.
    pub incremental_contribution: Rat,
    pub constrained_blend: Option<BlendAllocation>,
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

fn interpolate(x: Rat, (x0, y0): (Rat, Rat), (x1, y1): (Rat, Rat)) -> Result<Rat, Refusal> {
    if x == x0 {
        return Ok(y0);
    }
    if x == x1 {
        return Ok(y1);
    }
    x.checked_sub(x0)
        .and_then(|increase| increase.checked_div(x1.checked_sub(x0)?))
        .and_then(|fraction| fraction.checked_mul(y1.checked_sub(y0)?))
        .and_then(|offset| y0.checked_add(offset))
        .map_err(|_| arithmetic())
}

fn validate_constant(recovery: Rat) -> Result<(), Refusal> {
    if recovery.signum() <= 0 || recovery.num() > recovery.den() {
        return Err(domain("recovery must be a fraction in (0, 1]"));
    }
    Ok(())
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

    /// Recovered metal per dry tonne at a grade on the declared support.
    ///
    /// # Errors
    /// Refuses extrapolation and exact arithmetic overflow.
    pub fn recovered_grade(&self, grade: Rat) -> Result<Rat, Refusal> {
        self.check_band(grade, grade)?;
        for pair in self.knots.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if !compare(grade, right.grade)?.is_gt() {
                return interpolate(
                    grade,
                    (left.grade, left.recovered),
                    (right.grade, right.recovered),
                );
            }
        }
        Err(arithmetic())
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
            let grade = interpolate(
                required,
                (left.recovered, left.grade),
                (right.recovered, right.grade),
            )?;
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
        validate_constant(recovery)?;
        self.map_limits(|cost, margin| {
            margin
                .checked_mul(recovery)
                .and_then(|denom| cost.checked_div(denom))
                .map_err(|_| arithmetic())
        })
    }

    /// Screen the grade-tonnage population from highest to lowest grade.
    /// At a cap, the boundary bin may contribute a proportional share.
    /// The economic screen applies both caps and leaves break-even feed idle.
    ///
    /// # Errors
    /// Refuses missing, unordered or nonphysical inputs, extrapolation, and
    /// arithmetic outside the exact range.
    pub fn balance_feed(
        self,
        bins: &[GradeTonnageBin],
        max_grade: Rat,
        recovery: GradeRecovery<'_>,
    ) -> Result<CapacityBalance, Refusal> {
        self.screen_feed(bins, max_grade, recovery, None)
    }

    /// Also allocate two parcels under the declared additive blend limits.
    ///
    /// # Errors
    /// Refuses invalid feed or blend limits, and exact overflow.
    pub fn balance_feed_with_constraints(
        self,
        bins: &[GradeTonnageBin],
        max_grade: Rat,
        recovery: GradeRecovery<'_>,
        limits: BlendConstraints,
    ) -> Result<CapacityBalance, Refusal> {
        self.screen_feed(bins, max_grade, recovery, Some(limits))
    }

    fn screen_feed(
        self,
        bins: &[GradeTonnageBin],
        max_grade: Rat,
        recovery: GradeRecovery<'_>,
        constraints: Option<BlendConstraints>,
    ) -> Result<CapacityBalance, Refusal> {
        if !(1..=MAX_GRADE_BINS).contains(&bins.len()) {
            return Err(domain("grade-tonnage inventory requires 1 to 128 bins"));
        }
        let mill = self.mill_dry_tonnes_per_period;
        let market = self.market_metal_units_per_period;
        let net_price = self.net_price()?;
        if let GradeRecovery::Constant(fraction) = recovery {
            validate_constant(fraction)?;
        }
        let zero = Rat::from_integer(0);
        let mut available_dry_tonnes = zero;
        let mut available_recovered_metal = zero;
        let mut previous = None;
        let mut evaluated = Vec::with_capacity(bins.len());
        for &bin in bins {
            if bin.dry_tonnes.signum() <= 0
                || bin.grade.signum() < 0
                || compare(bin.grade, max_grade)?.is_gt()
            {
                return Err(domain(
                    "grade-tonnage bins require positive dry tonnes and a physical grade",
                ));
            }
            if let Some(grade) = previous
                && (compare(bin.grade, grade)?.is_lt()
                    || (grade == bin.grade && constraints.is_none()))
            {
                return Err(domain(
                    "grade-tonnage grades must increase, with ties allowed for a constrained blend",
                ));
            }
            previous = Some(bin.grade);
            let recovered = match recovery {
                GradeRecovery::Constant(fraction) => {
                    bin.grade.checked_mul(fraction).map_err(|_| arithmetic())?
                }
                GradeRecovery::Curve(curve) => curve.recovered_grade(bin.grade)?,
            };
            available_dry_tonnes = available_dry_tonnes
                .checked_add(bin.dry_tonnes)
                .map_err(|_| arithmetic())?;
            available_recovered_metal = recovered
                .checked_mul(bin.dry_tonnes)
                .and_then(|metal| available_recovered_metal.checked_add(metal))
                .map_err(|_| arithmetic())?;
            evaluated.push((bin, recovered));
        }
        let milling_cost = self.milling_cost_per_dry_tonne;
        let break_even = milling_cost
            .checked_div(net_price)
            .map_err(|_| arithmetic())?;
        // DESIGN: With nondecreasing recovered grade and uniform milling cost,
        // higher grades earn more per tonne and consume fewer tonnes per unit
        // of recovered metal. Positive bins therefore form an optimal prefix
        // in descending grade order under the two upper bounds.
        let mut first_profitable = evaluated.len();
        for &(_, recovered) in evaluated.iter().rev() {
            if !compare(recovered, break_even)?.is_gt() {
                break;
            }
            first_profitable -= 1;
        }
        let positive_contribution_feed =
            allocate_by_grade(&evaluated[first_profitable..], Some(mill), Some(market))?;
        let incremental_contribution = self.contribution(
            net_price,
            positive_contribution_feed.dry_tonnes,
            positive_contribution_feed.recovered_metal,
        )?;
        Ok(CapacityBalance {
            available_dry_tonnes,
            available_recovered_metal,
            mill_only: allocate_by_grade(&evaluated, Some(mill), None)?,
            market_only: allocate_by_grade(&evaluated, None, Some(market))?,
            both: allocate_by_grade(&evaluated, Some(mill), Some(market))?,
            positive_contribution_feed,
            incremental_contribution,
            constrained_blend: constraints
                .map(|limits| blend::solve(&evaluated, limits, self, net_price, max_grade))
                .transpose()?,
        })
    }

    fn contribution(self, net_price: Rat, dry_tonnes: Rat, metal: Rat) -> Result<Rat, Refusal> {
        let spend = self
            .milling_cost_per_dry_tonne
            .checked_mul(dry_tonnes)
            .map_err(|_| arithmetic())?;
        net_price
            .checked_mul(metal)
            .and_then(|revenue| revenue.checked_sub(spend))
            .map_err(|_| arithmetic())
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

    fn net_price(self) -> Result<Rat, Refusal> {
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
        Ok(margin)
    }

    fn map_limits<T>(
        self,
        grade: impl Fn(Rat, Rat) -> Result<T, Refusal>,
    ) -> Result<LaneCutoffs<T>, Refusal> {
        let margin = self.net_price()?;
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

fn allocate_by_grade(
    bins: &[(GradeTonnageBin, Rat)],
    mill: Option<Rat>,
    market: Option<Rat>,
) -> Result<GradeAllocation, Refusal> {
    let zero = Rat::from_integer(0);
    let mut allocation = GradeAllocation {
        dry_tonnes: zero,
        recovered_metal: zero,
        boundary: None,
    };
    for &(bin, recovered) in bins.iter().rev() {
        let mut take = bin.dry_tonnes;
        if let Some(cap) = mill {
            let slack = cap
                .checked_sub(allocation.dry_tonnes)
                .map_err(|_| arithmetic())?;
            if compare(take, slack)?.is_gt() {
                take = slack;
            }
        }
        if let Some(cap) = market {
            let slack = cap
                .checked_sub(allocation.recovered_metal)
                .map_err(|_| arithmetic())?;
            let metal = take.checked_mul(recovered).map_err(|_| arithmetic())?;
            if compare(metal, slack)?.is_gt() {
                take = slack.checked_div(recovered).map_err(|_| arithmetic())?;
            }
        }
        if take.signum() == 0 {
            break;
        }
        allocation.dry_tonnes = allocation
            .dry_tonnes
            .checked_add(take)
            .map_err(|_| arithmetic())?;
        allocation.recovered_metal = take
            .checked_mul(recovered)
            .and_then(|metal| allocation.recovered_metal.checked_add(metal))
            .map_err(|_| arithmetic())?;
        allocation.boundary = Some(GradeBoundary {
            grade: bin.grade,
            fraction: take.checked_div(bin.dry_tonnes).map_err(|_| arithmetic())?,
        });
        if take != bin.dry_tonnes {
            break;
        }
    }
    Ok(allocation)
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
    fn grade_capacity_boundary_handles_exact_caps_and_zero_recovery() {
        let terms = LaneTerms {
            price_per_metal_unit: qty("100"),
            selling_cost_per_metal_unit: qty("0"),
            milling_cost_per_dry_tonne: qty("20"),
            fixed_cost_per_period: qty("0"),
            opportunity_cost_per_period: qty("0"),
            mill_dry_tonnes_per_period: qty("1"),
            market_metal_units_per_period: qty("0.6"),
        };
        let bins = [
            GradeTonnageBin {
                grade: qty("0"),
                dry_tonnes: qty("2"),
            },
            GradeTonnageBin {
                grade: qty("1"),
                dry_tonnes: qty("1"),
            },
        ];
        let recovery = GradeRecovery::Constant(qty("0.6"));
        let result = terms.balance_feed(&bins, qty("1"), recovery).unwrap();
        assert_eq!(result.both, result.mill_only);
        assert_eq!(result.both.dry_tonnes, qty("1"));
        assert_eq!(result.both.recovered_metal, qty("0.6"));
        assert_eq!(
            result.both.boundary,
            Some(GradeBoundary {
                grade: qty("1"),
                fraction: qty("1")
            })
        );
        // Market-only can also admit the zero-metal bin: the prefix is
        // intact and recovered metal remains exactly at the cap.
        assert_eq!(result.market_only.dry_tonnes, qty("3"));
        assert_eq!(result.market_only.recovered_metal, qty("0.6"));
        assert_eq!(result.market_only.boundary.unwrap().grade, qty("0"));
        assert_eq!(result.positive_contribution_feed, result.both);
        assert_eq!(result.incremental_contribution, qty("40"));
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
