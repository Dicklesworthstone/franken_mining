//! DESIGN (BD-064/BD-333): signed copper cost and sold-volume bridges in USD per pound.
//! Determinism: DeterministicPerIsa (exact rational points; outward floating rounding enclosures).
//!
//! Both periods name the same C1 or AISC definition and ownership basis. USD millions and explicitly
//! typed copper sales are normalized using the statutory avoirdupois conversion in fmine-units.
//! This is an additive accounting bridge: no physical attribution, inferred zero line, gold-ounce
//! conversion or reconciliation plug. The caller authenticates sources and the accounting perimeter.

use std::collections::BTreeMap;

use fmine_numerics::exact_feas::Rat;
use fmine_units::{AVDP_LB_TO_GRAMS_DEN, AVDP_LB_TO_GRAMS_NUM};

use super::{
    Bar, BarPoint, BasisTag, Calc, Iv, ONE, PrintedQty, Refusal, RefusalCode, TWO, ZERO, bar,
    bounds, check_basis, exact, locators, numerical,
};

pub const RULES_VERSION: &str = "copper-additive-waterfall/1";
pub const RATE_UNIT: &str = "usd_per_lb";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    C1,
    Aisc,
}

/// Every quantity is copper sold on the period's declared ownership basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalesUnit {
    Pounds,
    MillionPounds,
    Kilotonnes,
}

impl SalesUnit {
    fn to_million_pounds(self) -> Calc<Rat> {
        match self {
            Self::Pounds => exact(Rat::new(1, 1_000_000)),
            Self::MillionPounds => Ok(ONE),
            // 1 kt = 10^9 g; one million lb = 10^6 * 453.59237 g.
            Self::Kilotonnes => exact(Rat::new(1000 * AVDP_LB_TO_GRAMS_DEN, AVDP_LB_TO_GRAMS_NUM)),
        }
    }
}

/// One declared copper numerator and its corresponding sold-metal population.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub metric: Metric,
    /// Registered definition id and version, or an authenticated equivalent definition tag.
    pub definition: String,
    /// USD millions; signed totals remain signed.
    pub cost: PrintedQty,
    /// Signed USD-million contributions with stable accounting identities.
    pub lines: Vec<(String, PrintedQty)>,
    pub sold: PrintedQty,
    pub sold_unit: SalesUnit,
    /// Optional independently reported USD/lb rate. Absence never becomes a manufactured observation.
    pub reported_rate: Option<PrintedQty>,
    pub basis: BasisTag,
}

/// Exact rates and optional independently reported-rate gaps, all in USD/lb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateComparison {
    pub rate_0: Rat,
    pub rate_1: Rat,
    pub total_delta: Rat,
    pub reported_rate_0: Option<Rat>,
    pub reported_rate_1: Option<Rat>,
    pub reported_rate_gap_0: Option<Rat>,
    pub reported_rate_gap_1: Option<Rat>,
    pub reported_rate_gap: Option<Rat>,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bridge {
    pub metric: Metric,
    pub definition: String,
    /// Normalized sold populations retain the original source locator and scaled rounding radius.
    pub sold_0_million_pounds: PrintedQty,
    pub sold_1_million_pounds: PrintedQty,
    /// Every bar is in RATE_UNIT, never USD per gold ounce.
    pub bars: Vec<Bar>,
    pub rates: RateComparison,
}

fn incompatible(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::BasisMismatch105, detail)
}

fn unidentified(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::NonIdentifiableAttribution104, detail)
}

fn sales(period: &Period) -> Calc<PrintedQty> {
    if bounds(&period.sold)?[0].signum() <= 0 {
        return Err(unidentified(
            "copper sold quantity must have a strictly positive rounding interval",
        ));
    }
    let scale = period.sold_unit.to_million_pounds()?;
    Ok(PrintedQty {
        value: exact(period.sold.value.checked_mul(scale))?,
        half_step: exact(period.sold.half_step.checked_mul(scale))?,
        at: period.sold.at.clone(),
    })
}

fn lines(period: &Period) -> Calc<BTreeMap<&str, &PrintedQty>> {
    bounds(&period.cost)?;
    if let Some(rate) = &period.reported_rate {
        bounds(rate)?;
    }
    let mut by_id = BTreeMap::new();
    for (id, amount) in &period.lines {
        bounds(amount)?;
        if id.trim().is_empty() || by_id.insert(id.as_str(), amount).is_some() {
            return Err(unidentified(
                "copper cost lines have an empty or duplicate identity",
            ));
        }
    }
    let total = by_id
        .values()
        .try_fold(ZERO, |sum, amount| exact(sum.checked_add(amount.value)))?;
    if by_id.is_empty() || total != period.cost.value {
        return Err(unidentified(
            "copper signed cost lines do not equal the supplied total; no plug",
        ));
    }
    Ok(by_id)
}

/// Compute the symmetric additive decomposition of cost divided by sold pounds.
/// Each line contributes delta(cost_line) * mean(1/sold); the sold-volume effect is
/// mean(total_cost) * delta(1/sold). The bars close exactly at the supplied midpoints.
/// Rounding enclosures are conservative independent input boxes, not confidence intervals.
pub fn decompose(p0: &Period, p1: &Period) -> Calc<Bridge> {
    check_basis(&p0.basis)?;
    check_basis(&p1.basis)?;
    if p0.basis != p1.basis
        || p0.metric != p1.metric
        || p0.definition.trim().is_empty()
        || p0.definition != p1.definition
    {
        return Err(incompatible(
            "copper waterfall requires identical metric, definition and ownership basis",
        ));
    }
    let (lines0, lines1) = (lines(p0)?, lines(p1)?);
    if !lines0.keys().eq(lines1.keys()) {
        return Err(unidentified(
            "copper cost line populations differ; a missing line is not an observed zero",
        ));
    }
    let (sold0, sold1) = (sales(p0)?, sales(p1)?);
    let rate_0 = exact(p0.cost.value.checked_div(sold0.value))?;
    let rate_1 = exact(p1.cost.value.checked_div(sold1.value))?;
    let total_delta = exact(rate_1.checked_sub(rate_0))?;
    let gap0 = p0
        .reported_rate
        .as_ref()
        .map(|rate| exact(rate.value.checked_sub(rate_0)))
        .transpose()?;
    let gap1 = p1
        .reported_rate
        .as_ref()
        .map(|rate| exact(rate.value.checked_sub(rate_1)))
        .transpose()?;
    let mut dependencies = vec![
        p0.cost.at.as_str(),
        p1.cost.at.as_str(),
        p0.sold.at.as_str(),
        p1.sold.at.as_str(),
    ];
    for rate in [&p0.reported_rate, &p1.reported_rate].into_iter().flatten() {
        dependencies.push(&rate.at);
    }
    let rates = RateComparison {
        rate_0,
        rate_1,
        total_delta,
        reported_rate_0: p0.reported_rate.as_ref().map(|rate| rate.value),
        reported_rate_1: p1.reported_rate.as_ref().map(|rate| rate.value),
        reported_rate_gap_0: gap0,
        reported_rate_gap_1: gap1,
        reported_rate_gap: gap0
            .zip(gap1)
            .map(|(first, second)| exact(second.checked_sub(first)))
            .transpose()?,
        inputs: locators(dependencies),
    };
    let inverse0 = exact(ONE.checked_div(sold0.value))?;
    let inverse1 = exact(ONE.checked_div(sold1.value))?;
    let mean_inverse = exact(exact(inverse0.checked_add(inverse1))?.checked_div(TWO))?;
    let delta_inverse = exact(inverse1.checked_sub(inverse0))?;
    let inverse_box0 = Iv::point(1.0)?.div(Iv::qty(&sold0)?)?;
    let inverse_box1 = Iv::point(1.0)?.div(Iv::qty(&sold1)?)?;
    let mean_box = inverse_box0.add(inverse_box1)?.mul(Iv::point(0.5)?)?;
    let delta_box = inverse_box1.sub(inverse_box0)?;
    let mut bars = Vec::with_capacity(lines0.len() + 1);
    let mut total = ZERO;
    for ((id, first), (_, second)) in lines0.iter().zip(&lines1) {
        let change = exact(second.value.checked_sub(first.value))?;
        let point = exact(change.checked_mul(mean_inverse))?;
        total = exact(total.checked_add(point))?;
        bars.push(bar(
            &format!("line:{id}"),
            "copper.waterfall.additive.line",
            BarPoint::Exact(point),
            Iv::qty(second)?.sub(Iv::qty(first)?)?.mul(mean_box)?,
            locators([first.at.as_str(), &second.at, &p0.sold.at, &p1.sold.at]),
        )?);
    }
    let mean_cost = exact(exact(p0.cost.value.checked_add(p1.cost.value))?.checked_div(TWO))?;
    let point = exact(mean_cost.checked_mul(delta_inverse))?;
    total = exact(total.checked_add(point))?;
    bars.push(bar(
        "sold_volume",
        "copper.waterfall.additive.sold_volume",
        BarPoint::Exact(point),
        Iv::qty(&p0.cost)?
            .add(Iv::qty(&p1.cost)?)?
            .mul(Iv::point(0.5)?)?
            .mul(delta_box)?,
        locators([
            p0.cost.at.as_str(),
            p1.cost.at.as_str(),
            p0.sold.at.as_str(),
            p1.sold.at.as_str(),
        ]),
    )?);
    if total != total_delta {
        return Err(numerical("copper waterfall exact bars do not close"));
    }
    Ok(Bridge {
        metric: p0.metric,
        definition: p0.definition.clone(),
        sold_0_million_pounds: sold0,
        sold_1_million_pounds: sold1,
        bars,
        rates,
    })
}

#[cfg(test)]
mod tests;
