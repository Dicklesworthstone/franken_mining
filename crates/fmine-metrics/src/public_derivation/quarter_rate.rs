//! DESIGN (BD-059/345): sales-weighted quarterly rates from two cumulative periods.
//! Determinism: Exact
//! The same sales variable occurs in numerator and denominator; independent interval division would
//! lose that dependence. A positive denominator makes the ratio linear-fractional in each coordinate,
//! so its extrema over the four printed rounding boxes occur at vertices.

use super::{
    AdditivityClass, Claim, CostRateUnit, EvidenceClass, Interval, Period, Printed, RefusalCode,
    Representation, Result, SpatialSupport, checked, refusal, validate_source,
};

pub const RULE: &str = "public-disclosure.sales-weighted-quarter-rate/1";
pub const ASSUMPTION: &str = "Both cumulative rates use the same issuer-reported cost definition and matching sold-gold denominator in one filing. The quarter numerator is the difference of implied cumulative costs; credit treatment and accounting-definition equivalence are not independently established.";
pub const COPPER_RULE: &str = "public-disclosure.copper-sales-weighted-quarter-rate/1";
pub const COPPER_ASSUMPTION: &str = "Both cumulative rates use the same issuer-reported cost definition and matching sold-copper denominator in millions of pounds in one filing. The quarter numerator is the difference of implied cumulative costs; credit treatment and accounting-definition equivalence are not independently established. No production denominator or tonne/pound conversion is inferred.";

/// The rate family fixes the sold-metal unit; the common cumulative sales scale cancels.
#[must_use]
pub fn family_unit(family: &str) -> Option<CostRateUnit> {
    match family {
        "aisc_per_oz" | "tcc_per_oz" | "cos_per_oz" => Some(CostRateUnit::GoldKoz),
        "aisc_per_lb" | "c1_per_lb" | "cos_per_lb" => Some(CostRateUnit::CopperMillionPounds),
        _ => None,
    }
}

#[must_use]
pub const fn rule(unit: CostRateUnit) -> &'static str {
    match unit {
        CostRateUnit::GoldKoz => RULE,
        CostRateUnit::CopperMillionPounds => COPPER_RULE,
    }
}

#[must_use]
pub const fn assumption(unit: CostRateUnit) -> &'static str {
    match unit {
        CostRateUnit::GoldKoz => ASSUMPTION,
        CostRateUnit::CopperMillionPounds => COPPER_ASSUMPTION,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrintedPeriod {
    pub rate: Printed,
    pub sales: Printed,
}

/// Exact closed enclosure of `(current.rate*current.sales - prior.rate*prior.sales) /
/// (current.sales - prior.sales)`. Both sales boxes and the entire difference must be positive.
/// Rates may be signed; the common sales scale cancels and no unit conversion occurs.
pub fn bounds(prior: PrintedPeriod, current: PrintedPeriod) -> Result<Interval> {
    let boxes = [prior.rate, prior.sales, current.rate, current.sales].map(|value| {
        value.interval().map_err(|_| {
            refusal(
                RefusalCode::NumericalCertificateFailed207,
                "invalid or overflowing printed cost-rate enclosure",
            )
        })
    });
    let [pr, pq, cr, cq] = boxes;
    let [pr, pq, cr, cq] = [pr?, pq?, cr?, cq?];
    if pq.lo.signum() <= 0
        || cq.lo.signum() <= 0
        || checked(cq.lo.checked_sub(pq.hi))?.signum() <= 0
    {
        return Err(refusal(
            RefusalCode::NonIdentifiableAttribution104,
            "cumulative and incremental sold-metal enclosures must be strictly positive",
        ));
    }
    let mut range: Option<Interval> = None;
    for a in [pr.lo, pr.hi] {
        for b in [pq.lo, pq.hi] {
            for c in [cr.lo, cr.hi] {
                for d in [cq.lo, cq.hi] {
                    let numerator =
                        checked(c.checked_mul(d))?.checked_sub(checked(a.checked_mul(b))?);
                    let value =
                        checked(checked(numerator)?.checked_div(checked(d.checked_sub(b))?))?;
                    if let Some(interval) = &mut range {
                        if checked(value.checked_cmp(interval.lo))?.is_lt() {
                            interval.lo = value;
                        }
                        if checked(value.checked_cmp(interval.hi))?.is_gt() {
                            interval.hi = value;
                        }
                    } else {
                        range = Some(Interval {
                            lo: value,
                            hi: value,
                            lo_strict: false,
                            hi_strict: false,
                        });
                    }
                }
            }
        }
    }
    range.ok_or_else(|| {
        refusal(
            RefusalCode::NumericalCertificateFailed207,
            "cost-rate enclosure has no checked vertices",
        )
    })
}

/// Four canonical source Claims in role order: prior rate, prior sales, current rate, current sales.
/// The adapter checks that printed values, calendar support, issuer, site, ownership and filing match.
/// This constructor retains all four parents and never upgrades their evidence or calibration state.
pub fn derive<T>(
    family: &str,
    target: Period,
    sources: [&Claim<T>; 4],
    prior: PrintedPeriod,
    current: PrintedPeriod,
) -> Result<Claim<Interval>> {
    let unit = family_unit(family).ok_or_else(|| {
        refusal(
            RefusalCode::BasisMismatch105,
            "sales-weighted quarter requires a gold AISC/TCC/cost-of-sales or copper AISC/C1/cost-of-sales family",
        )
    })?;
    if target.year == 0
        || !matches!(
            (target.first_month, target.last_month),
            (4, 6) | (7, 9) | (10, 12)
        )
    {
        return Err(refusal(
            RefusalCode::BasisMismatch105,
            "sales-weighted quarter requires Q2, Q3 or Q4",
        ));
    }
    let first = sources[0];
    let mut evidence = first.evidence.meet(EvidenceClass::Derived);
    let mut epistemic = first.epistemic;
    let mut state = first.state;
    let mut confidentiality = first.confidentiality().clone();
    let (mut terms, mut parents, mut anchors) = (Vec::new(), Vec::new(), Vec::new());
    for (index, source) in sources.into_iter().enumerate() {
        validate_source(source, &source.metric_key, source.value_tag())?;
        let expected = if index.is_multiple_of(2) {
            AdditivityClass::IntensiveWeighted
        } else {
            AdditivityClass::ExtensiveAdditive
        };
        if source.additivity != expected || parents.contains(&source.claim_id) {
            return Err(refusal(
                RefusalCode::NonAdditiveAverage103,
                "cost-rate operands require distinct weighted rates and extensive sold-metal quantities",
            ));
        }
        evidence = evidence.meet(source.evidence);
        epistemic = epistemic.and(source.epistemic);
        state = state.and(source.state);
        confidentiality = confidentiality.join(source.confidentiality());
        parents.push(source.claim_id);
        anchors.extend(source.anchors.iter().cloned());
        for term in &source.uu_terms {
            if !terms.contains(term) {
                terms.push(term.clone());
            }
        }
    }
    let interval = bounds(prior, current)?;
    let tag = format!(
        "{family}:{}:{}-{}:{}/{}:{}/{}:{}",
        target.year,
        target.first_month,
        target.last_month,
        interval.lo.num(),
        interval.lo.den(),
        interval.hi.num(),
        interval.hi.den(),
        assumption(unit)
    );
    anchors.sort();
    anchors.dedup();
    Ok(Claim::new(
        rule(unit),
        interval,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::IntensiveWeighted,
        evidence,
        epistemic,
        state,
        confidentiality,
        terms,
        parents,
        anchors,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_numerics::exact_feas::Rat;

    fn period(rate: i128, sales: i128) -> PrintedPeriod {
        PrintedPeriod {
            rate: Printed {
                value: Rat::from_integer(rate),
                step: Rat::from_integer(1),
            },
            sales: Printed {
                value: Rat::from_integer(sales),
                step: Rat::from_integer(2),
            },
        }
    }

    #[test]
    fn exact_weighted_quarter_preserves_shared_sales_dependence_and_signed_rates() {
        // Independent monotonic form: current_rate + (current_rate-prior_rate)*prior_sales/delta_sales.
        // Minimum = 19.5 + 9*99/102; maximum = 20.5 + 11*101/98.
        let result = bounds(period(10, 100), period(20, 200)).unwrap();
        assert_eq!(result.lo, Rat::new(480, 17).unwrap());
        assert_eq!(result.hi, Rat::new(1560, 49).unwrap());
        let negative = bounds(period(-10, 100), period(-20, 200)).unwrap();
        assert_eq!(negative.lo, Rat::new(-1560, 49).unwrap());
        assert_eq!(negative.hi, Rat::new(-480, 17).unwrap());
        let mut scaled = [period(10, 200), period(20, 400)];
        for item in &mut scaled {
            item.sales.step = Rat::from_integer(4);
        }
        assert_eq!(bounds(scaled[0], scaled[1]).unwrap(), result);
    }

    #[test]
    fn uncertain_zero_increment_unknown_precision_and_overflow_refuse() {
        for current in [period(20, 99), period(20, 100), period(20, 102)] {
            assert_eq!(
                bounds(period(10, 100), current).unwrap_err().code,
                "FMINE-REF-104"
            );
        }
        assert_eq!(
            bounds(period(10, 0), period(20, 200)).unwrap_err().code,
            "FMINE-REF-104"
        );
        let mut prior = period(10, 100);
        prior.rate.step = Rat::from_integer(0);
        assert_eq!(
            bounds(prior, period(20, 200)).unwrap_err().code,
            "FMINE-REF-207"
        );
        prior = period(i128::MAX, 100);
        assert_eq!(
            bounds(prior, period(20, 200)).unwrap_err().code,
            "FMINE-REF-207"
        );
    }
}
