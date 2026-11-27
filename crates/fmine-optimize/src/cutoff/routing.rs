//! DESIGN (BD-252, plan §15.6): exact routing and carry of already-mined parcels.
//! Determinism: Exact

use std::collections::BTreeSet;

use fmine_numerics::exact_feas::{LinearMaximumError, maximize_nonnegative};

use super::{Rat, Refusal, RefusalCode, compare, domain};

pub const MAX_PARCELS: usize = 8;
pub const MAX_ROUTES: usize = 4;
pub const MAX_PERIODS: usize = 4;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);

#[derive(Clone, Copy, Debug)]
pub struct Parcel {
    pub available_dry_tonnes: Rat,
    pub grade_per_dry_tonne: Rat,
    pub impurity_mass_fraction: Option<Rat>,
}

#[derive(Clone, Copy, Debug)]
pub struct Route {
    pub dry_tonnes_cap: Rat,
    pub milling_cost_per_dry_tonne: Rat,
    pub energy_budget_kwh: Option<Rat>,
    pub impurity_ceiling: Option<Rat>,
}

/// One physically eligible parcel-to-route pair. Omitted pairs are unavailable.
#[derive(Clone, Copy, Debug)]
pub struct RoutePair {
    pub parcel: usize,
    pub route: usize,
    pub recovery_fraction: Rat,
    pub haul_cost_per_dry_tonne: Rat,
    pub specific_energy_kwh_per_dry_tonne: Option<Rat>,
}

/// One metal, currency, period and project basis. The grade limit is one dry tonne of contained metal
/// expressed in the declared unit. Receipts use the common recovered-metal price.
#[derive(Clone, Copy, Debug)]
pub struct Scenario<'a> {
    pub parcels: &'a [Parcel],
    pub routes: &'a [Route],
    pub pairs: &'a [RoutePair],
    pub physical_grade_limit: Rat,
    pub market_recovered_metal_cap: Rat,
    pub price_per_metal_unit: Rat,
    pub selling_cost_per_metal_unit: Rat,
}

#[derive(Clone, Copy, Debug)]
pub struct Period<'a> {
    pub routes: &'a [Route],
    pub pairs: &'a [RoutePair],
    /// Arrivals before the period's routing, in shared parcel order. An empty slice means zero arrivals.
    pub arrivals_dry_tonnes: &'a [Rat],
    pub discount_factor: Rat,
    pub market_recovered_metal_cap: Rat,
    pub price_per_metal_unit: Rat,
    pub selling_cost_per_metal_unit: Rat,
}

#[derive(Clone, Copy, Debug)]
pub struct Horizon<'a> {
    pub parcels: &'a [Parcel],
    pub periods: &'a [Period<'a>],
    /// End-of-period storage caps by parcel. An empty slice means every parcel can carry without a cap.
    pub stockpile_caps_dry_tonnes: &'a [Option<Rat>],
    pub physical_grade_limit: Rat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub dry_tonnes: Rat,
    pub recovered_metal: Rat,
    pub contribution: Rat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteResult {
    pub allocation: Allocation,
    pub energy_used_kwh: Option<Rat>,
    pub blended_impurity_mass_fraction: Option<Rat>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Routing {
    /// One result for each supplied pair, in input order.
    pub pairs: Vec<Allocation>,
    pub routes: Vec<RouteResult>,
    pub unallocated_dry_tonnes: Vec<Rat>,
    pub total: Allocation,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HorizonRouting {
    /// Each routing's unallocated dry tonnes are that period's closing stock.
    pub periods: Vec<Routing>,
    pub total: Allocation,
    pub discounted_contribution: Rat,
}

fn numerical(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

fn arithmetic() -> Refusal {
    numerical("route blend exceeds exact arithmetic range")
}

fn add(a: Rat, b: Rat) -> Result<Rat, Refusal> {
    a.checked_add(b).map_err(|_| arithmetic())
}

fn sub(a: Rat, b: Rat) -> Result<Rat, Refusal> {
    a.checked_sub(b).map_err(|_| arithmetic())
}

fn mul(a: Rat, b: Rat) -> Result<Rat, Refusal> {
    a.checked_mul(b).map_err(|_| arithmetic())
}

fn nonnegative(values: impl IntoIterator<Item = Rat>) -> bool {
    values.into_iter().all(|value| value.signum() >= 0)
}

fn mass_fraction(value: Rat) -> Result<bool, Refusal> {
    Ok(value.signum() >= 0 && !compare(value, ONE)?.is_gt())
}

impl Allocation {
    const ZERO: Self = Self {
        dry_tonnes: ZERO,
        recovered_metal: ZERO,
        contribution: ZERO,
    };

    fn add(&mut self, other: Self) -> Result<(), Refusal> {
        self.dry_tonnes = add(self.dry_tonnes, other.dry_tonnes)?;
        self.recovered_metal = add(self.recovered_metal, other.recovered_metal)?;
        self.contribution = add(self.contribution, other.contribution)?;
        Ok(())
    }
}

/// Route divisible feed to maximize recovered-metal revenue less selling, milling and haul costs.
/// Unallocated feed stays available. An optimum with zero contribution selects idle feed.
///
/// # Errors
/// Refuses invalid dimensions, pair keys, physical or monetary values and uncertified exact solutions.
pub fn solve(scenario: Scenario<'_>) -> Result<Routing, Refusal> {
    if scenario.routes.is_empty() {
        return Err(domain("one-period route blend requires at least one route"));
    }
    let period = Period {
        routes: scenario.routes,
        pairs: scenario.pairs,
        arrivals_dry_tonnes: &[],
        discount_factor: ONE,
        market_recovered_metal_cap: scenario.market_recovered_metal_cap,
        price_per_metal_unit: scenario.price_per_metal_unit,
        selling_cost_per_metal_unit: scenario.selling_cost_per_metal_unit,
    };
    solve_horizon(Horizon {
        parcels: scenario.parcels,
        periods: &[period],
        stockpile_caps_dry_tonnes: &[],
        physical_grade_limit: scenario.physical_grade_limit,
    })?
    .periods
    .into_iter()
    .next()
    .ok_or_else(|| numerical("exact route blend did not return a period"))
}

fn validate_parcels(parcels: &[Parcel], physical_grade_limit: Rat) -> Result<bool, Refusal> {
    if !(1..=MAX_PARCELS).contains(&parcels.len()) || physical_grade_limit.signum() <= 0 {
        return Err(domain(
            "route blend requires 1 to 8 parcels and a positive physical grade limit",
        ));
    }
    let has_assay = parcels.iter().any(|p| p.impurity_mass_fraction.is_some());
    for p in parcels {
        if !nonnegative([p.available_dry_tonnes, p.grade_per_dry_tonne])
            || compare(p.grade_per_dry_tonne, physical_grade_limit)?.is_gt()
        {
            return Err(domain(
                "parcel availability and grade must be nonnegative with a physical grade",
            ));
        }
        if p.impurity_mass_fraction.is_some() != has_assay {
            return Err(domain(
                "a declared impurity requires assays on every parcel",
            ));
        }
        if let Some(assay) = p.impurity_mass_fraction {
            let metal = p
                .grade_per_dry_tonne
                .checked_div(physical_grade_limit)
                .map_err(|_| arithmetic())?;
            if !mass_fraction(assay)? || compare(add(assay, metal)?, ONE)?.is_gt() {
                return Err(domain(
                    "parcel impurity and contained metal must fit within dry mass",
                ));
            }
        }
    }
    Ok(has_assay)
}

struct PeriodModel {
    rows: Vec<Vec<Rat>>,
    rhs: Vec<Rat>,
    margins: Vec<Rat>,
    market_row: usize,
}

fn period_model(
    parcels: &[Parcel],
    has_assay: bool,
    period: Period<'_>,
) -> Result<PeriodModel, Refusal> {
    let Period {
        routes,
        pairs,
        market_recovered_metal_cap,
        price_per_metal_unit,
        selling_cost_per_metal_unit,
        ..
    } = period;
    if routes.len() > MAX_ROUTES || pairs.len() > parcels.len() * routes.len() {
        return Err(domain(
            "route blend permits at most four routes with one entry per eligible pair",
        ));
    }
    if !nonnegative([
        market_recovered_metal_cap,
        price_per_metal_unit,
        selling_cost_per_metal_unit,
    ]) {
        return Err(domain(
            "price, selling cost and market cap must be nonnegative",
        ));
    }
    let mut rhs = Vec::with_capacity(3 * routes.len() + 1);
    for route in routes {
        let budget = route.energy_budget_kwh.unwrap_or(ZERO);
        if !nonnegative([
            route.dry_tonnes_cap,
            route.milling_cost_per_dry_tonne,
            budget,
        ]) {
            return Err(domain(
                "route throughput, cost and energy budget must be nonnegative",
            ));
        }
        if let Some(ceiling) = route.impurity_ceiling
            && (!has_assay || !mass_fraction(ceiling)?)
        {
            return Err(domain(
                "route impurity ceiling requires complete assays and a fraction between zero and one",
            ));
        }
        rhs.extend([route.dry_tonnes_cap, budget, ZERO]);
    }
    let market_row = rhs.len();
    rhs.push(market_recovered_metal_cap);
    let mut rows = vec![vec![ZERO; pairs.len()]; rhs.len()];
    let mut margins = Vec::with_capacity(pairs.len());
    let mut keys = BTreeSet::new();
    let net_price = sub(price_per_metal_unit, selling_cost_per_metal_unit)?;
    for (j, pair) in pairs.iter().enumerate() {
        let (Some(parcel), Some(route)) = (parcels.get(pair.parcel), routes.get(pair.route)) else {
            return Err(domain("route pair names an unknown parcel or route"));
        };
        if !keys.insert((pair.parcel, pair.route)) {
            return Err(domain("duplicate parcel and route pair"));
        }
        let energy = pair.specific_energy_kwh_per_dry_tonne.unwrap_or(ZERO);
        if !mass_fraction(pair.recovery_fraction)?
            || !nonnegative([pair.haul_cost_per_dry_tonne, energy])
            || pair.specific_energy_kwh_per_dry_tonne.is_some() != route.energy_budget_kwh.is_some()
        {
            return Err(domain(
                "route pair requires physical recovery, nonnegative haul and specific energy exactly when a budget is present",
            ));
        }
        let recovered = mul(parcel.grade_per_dry_tonne, pair.recovery_fraction)?;
        let route_row = 3 * pair.route;
        rows[route_row][j] = ONE;
        rows[route_row + 1][j] = energy;
        if let (Some(assay), Some(ceiling)) =
            (parcel.impurity_mass_fraction, route.impurity_ceiling)
        {
            rows[route_row + 2][j] = sub(assay, ceiling)?;
        }
        rows[market_row][j] = recovered;
        let cost = add(
            route.milling_cost_per_dry_tonne,
            pair.haul_cost_per_dry_tonne,
        )?;
        margins.push(sub(mul(recovered, net_price)?, cost)?);
    }
    Ok(PeriodModel {
        rows,
        rhs,
        margins,
        market_row,
    })
}

/// Optimize each period jointly. Cumulative withdrawals can use only opening stock and arrivals already made.
/// A storage cap applies at period close. A zero-valued solution stays idle if idle is feasible.
///
/// # Errors
/// Refuses invalid periods, arrivals or caps, infeasible stock limits, and uncertified exact solutions.
pub fn solve_horizon(horizon: Horizon<'_>) -> Result<HorizonRouting, Refusal> {
    let Horizon {
        parcels,
        periods,
        stockpile_caps_dry_tonnes: caps,
        physical_grade_limit,
    } = horizon;
    let has_assay = validate_parcels(parcels, physical_grade_limit)?;
    if !(1..=MAX_PERIODS).contains(&periods.len())
        || (!caps.is_empty() && caps.len() != parcels.len())
        || !nonnegative(caps.iter().copied().flatten())
    {
        return Err(domain(
            "declare 1 to 4 periods and nonnegative storage caps in shared parcel order",
        ));
    }
    let mut models = Vec::with_capacity(periods.len());
    for &period in periods {
        if (!period.arrivals_dry_tonnes.is_empty()
            && period.arrivals_dry_tonnes.len() != parcels.len())
            || !nonnegative(period.arrivals_dry_tonnes.iter().copied())
            || !mass_fraction(period.discount_factor)?
        {
            return Err(domain(
                "declare nonnegative arrivals in shared parcel order and a discount factor in [0, 1]",
            ));
        }
        models.push(period_model(parcels, has_assay, period)?);
    }
    let column_parcels: Vec<_> = periods
        .iter()
        .flat_map(|p| p.pairs.iter().map(|pair| pair.parcel))
        .collect();
    let mut cumulative: Vec<_> = parcels.iter().map(|p| p.available_dry_tonnes).collect();
    let n = column_parcels.len();
    let mut rows = Vec::new();
    let mut rhs = Vec::new();
    let mut objective = Vec::with_capacity(n);
    let mut start = 0;
    for (period, model) in periods.iter().zip(&models) {
        let end = start + period.pairs.len();
        for (available, arriving) in cumulative.iter_mut().zip(period.arrivals_dry_tonnes) {
            *available = add(*available, *arriving)?;
        }
        for (parcel, available) in cumulative.iter().copied().enumerate() {
            let mut row = vec![ZERO; n];
            for (cell, &source) in row[..end].iter_mut().zip(&column_parcels) {
                if source == parcel {
                    *cell = ONE;
                }
            }
            if let Some(Some(cap)) = caps.get(parcel)
                && compare(available, *cap)?.is_gt()
            {
                rows.push(
                    row.iter()
                        .map(|value| {
                            if *value == ONE {
                                Rat::from_integer(-1)
                            } else {
                                ZERO
                            }
                        })
                        .collect(),
                );
                rhs.push(sub(*cap, available)?);
            }
            rows.push(row);
            rhs.push(available);
        }
        for (local, &bound) in model.rows.iter().zip(&model.rhs) {
            let mut row = vec![ZERO; n];
            row[start..end].copy_from_slice(local);
            rows.push(row);
            rhs.push(bound);
        }
        for &margin in &model.margins {
            objective.push(mul(margin, period.discount_factor)?);
        }
        start = end;
    }
    let optimum = maximize_nonnegative(&rows, &rhs, &objective).map_err(|error| match error {
        LinearMaximumError::Infeasible => {
            domain("no routing can satisfy the declared stockpile and processing caps")
        }
        LinearMaximumError::Uncertified(reason) => numerical(reason),
    })?;
    let idle = optimum.objective == ZERO && nonnegative(rhs);
    let mut stock: Vec<_> = parcels.iter().map(|p| p.available_dry_tonnes).collect();
    let mut total = Allocation::ZERO;
    let mut results = Vec::with_capacity(periods.len());
    let mut values = optimum.variables.as_slice();
    for (period, model) in periods.iter().zip(&models) {
        for (available, arriving) in stock.iter_mut().zip(period.arrivals_dry_tonnes) {
            *available = add(*available, *arriving)?;
        }
        let (selected, remaining) = values.split_at(period.pairs.len());
        values = remaining;
        let result = period_result(parcels, *period, model, stock, selected, idle)?;
        stock = result.unallocated_dry_tonnes.clone();
        total.add(result.total)?;
        results.push(result);
    }
    Ok(HorizonRouting {
        periods: results,
        total,
        discounted_contribution: optimum.objective,
    })
}

fn period_result(
    parcels: &[Parcel],
    period: Period<'_>,
    model: &PeriodModel,
    stock: Vec<Rat>,
    values: &[Rat],
    idle: bool,
) -> Result<Routing, Refusal> {
    let Period { routes, pairs, .. } = period;
    let mut result = Routing {
        pairs: Vec::with_capacity(pairs.len()),
        routes: routes
            .iter()
            .map(|route| RouteResult {
                allocation: Allocation::ZERO,
                energy_used_kwh: route.energy_budget_kwh.map(|_| ZERO),
                blended_impurity_mass_fraction: None,
            })
            .collect(),
        unallocated_dry_tonnes: stock,
        total: Allocation::ZERO,
    };
    let mut impurity_mass = vec![ZERO; routes.len()];
    for (j, (pair, &amount)) in pairs.iter().zip(values).enumerate() {
        let dry = if idle { ZERO } else { amount };
        let allocation = Allocation {
            dry_tonnes: dry,
            recovered_metal: mul(dry, model.rows[model.market_row][j])?,
            contribution: mul(dry, model.margins[j])?,
        };
        result.total.add(allocation)?;
        result.pairs.push(allocation);
        result.unallocated_dry_tonnes[pair.parcel] =
            sub(result.unallocated_dry_tonnes[pair.parcel], dry)?;
        let route = &mut result.routes[pair.route];
        route.allocation.add(allocation)?;
        if let (Some(used), Some(rate)) = (
            route.energy_used_kwh,
            pair.specific_energy_kwh_per_dry_tonne,
        ) {
            route.energy_used_kwh = Some(add(used, mul(dry, rate)?)?);
        }
        if let Some(assay) = parcels[pair.parcel].impurity_mass_fraction {
            impurity_mass[pair.route] = add(impurity_mass[pair.route], mul(dry, assay)?)?;
        }
    }
    if parcels
        .iter()
        .any(|parcel| parcel.impurity_mass_fraction.is_some())
    {
        for (route, assay_mass) in result.routes.iter_mut().zip(impurity_mass) {
            let dry = route.allocation.dry_tonnes;
            if dry.signum() > 0 {
                route.blended_impurity_mass_fraction =
                    Some(assay_mass.checked_div(dry).map_err(|_| arithmetic())?);
            }
        }
    }
    Ok(result)
}
